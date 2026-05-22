use crate::bump::PlannedBump;
use crate::config::OxrlsConfig;
use crate::error::{OxrlsError, Result};
use crate::package_json::PackageJson;
use crate::workspace::Workspace;
use indexmap::IndexMap;
use std::path::{Path, PathBuf};

/// A release manifest — just a sorted list of package names in a plain text file.
#[derive(Debug, Clone)]
pub struct ReleaseManifest {
  pub packages: Vec<String>,
}

impl ReleaseManifest {
  pub fn path(release_dir: &Path) -> PathBuf {
    release_dir.join("releaseplan.txt")
  }

  /// Build a manifest from a completed bump plan, extracting just the package names.
  pub fn from_bumps(bumps: &IndexMap<String, PlannedBump>) -> Self {
    let mut packages: Vec<String> = bumps.keys().cloned().collect();
    packages.sort();
    ReleaseManifest { packages }
  }

  /// Save the manifest as a plain text file (one package name per line).
  pub fn save(&self, release_dir: &Path) -> Result<()> {
    let path = Self::path(release_dir);
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent).map_err(OxrlsError::Io)?;
    }
    let content = self.packages.join("\n") + "\n";
    std::fs::write(&path, content).map_err(OxrlsError::Io)?;
    Ok(())
  }

  /// Load the manifest from a plain text file.
  pub fn load(release_dir: &Path) -> Result<Self> {
    let path = Self::path(release_dir);
    if !path.exists() {
      return Err(OxrlsError::Bump(format!(
        "No release plan found at {}. Run `oxrls bump` first.",
        path.display()
      )));
    }
    let content = std::fs::read_to_string(&path)
      .map_err(|e| OxrlsError::Bump(format!("Failed to read release plan: {}", e)))?;
    let packages: Vec<String> = content
      .lines()
      .map(|l| l.trim().to_string())
      .filter(|l| !l.is_empty())
      .collect();
    Ok(ReleaseManifest { packages })
  }

  /// Remove the manifest file after a successful release.
  pub fn remove(release_dir: &Path) -> Result<()> {
    let path = Self::path(release_dir);
    if path.exists() {
      std::fs::remove_file(&path).map_err(OxrlsError::Io)?;
    }
    Ok(())
  }
}

/// Extract the pre-release tag from a version string (e.g., "1.0.0-beta.1" → "beta").
fn extract_pre_tag(version: &str) -> Option<String> {
  let v = semver::Version::parse(version).ok()?;
  let pre_str = v.pre.as_str();
  if pre_str.is_empty() {
    return None;
  }
  pre_str.split_once('.').map(|(tag, _)| tag.to_string())
}

/// Check if a package version already exists on the npm registry.
fn check_version_exists(package_name: &str, version: &str) -> Result<bool> {
  let output = std::process::Command::new("npm")
    .args(["view", &format!("{}@{}", package_name, version), "version"])
    .output()
    .map_err(|e| OxrlsError::Bump(format!("Failed to run npm view: {}", e)))?;

  if output.status.success() {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let viewed = stdout.trim();
    if viewed == version || viewed.contains(version) {
      return Ok(true);
    }
  }
  Ok(false)
}

/// Get the registry URL from package.json publishConfig.
fn get_registry(pkg_json: &PackageJson) -> Option<String> {
  pkg_json.extra.get("publishConfig").and_then(|pc| {
    pc.as_object()
      .and_then(|obj| obj.get("registry"))
      .and_then(|v| v.as_str())
      .map(|s| s.to_string())
  })
}

/// Get the access level from package.json publishConfig, falling back to config.
fn get_access(pkg_json: &PackageJson, config: &OxrlsConfig) -> String {
  let from_publish_config = pkg_json.extra.get("publishConfig").and_then(|pc| {
    pc.as_object()
      .and_then(|obj| obj.get("access"))
      .and_then(|v| v.as_str())
  });
  from_publish_config
    .map(|s| s.to_string())
    .unwrap_or_else(|| config.access.to_string())
}

/// Publish all packages listed in the release manifest.
/// Reads the current `package.json` for each package to determine version info.
pub fn publish_manifest(
  manifest: &ReleaseManifest,
  workspace: &Workspace,
  config: &OxrlsConfig,
  dry_run: bool,
  tag_override: Option<&str>,
) -> Result<()> {
  let mut published = 0u32;
  let mut skipped = 0u32;

  for package_name in &manifest.packages {
    let pkg = workspace.packages.get(package_name).ok_or_else(|| {
      OxrlsError::Bump(format!(
        "Package \"{}\" not found in workspace.",
        package_name
      ))
    })?;

    let pkg_dir = &pkg.dir;
    let pkg_json = PackageJson::read(&pkg_dir.join("package.json"))?;
    let current_version = pkg_json.version.as_deref().unwrap_or("unknown");

    // Skip private packages
    if pkg_json.private.unwrap_or(false) {
      println!("  {} (private — skipped)", package_name);
      skipped += 1;
      continue;
    }

    // Determine the dist-tag: override > pre-release tag > "latest"
    let pre_tag = extract_pre_tag(current_version);
    let dist_tag = tag_override
      .or(pre_tag.as_deref())
      .unwrap_or("latest");

    let registry = get_registry(&pkg_json);

    println!("{} (v{}, {})", package_name, current_version, dist_tag);

    if let Some(reg) = registry {
      println!("  Registry: {}", reg);
    }

    // Check if version already exists on the registry
    if !dry_run {
      match check_version_exists(package_name, current_version) {
        Ok(true) => {
          println!("  Version {} already exists on registry — skipped.", current_version);
          skipped += 1;
          continue;
        }
        Ok(false) => {}
        Err(e) => {
          eprintln!(
            "  Warning: could not check registry ({}). Proceeding with publish.",
            e
          );
        }
      }
    }

    if dry_run {
      continue;
    }

    // Build the publish command
    let mut cmd = std::process::Command::new("npm");
    cmd.arg("publish");
    cmd.current_dir(pkg_dir);
    cmd.arg("--tag");
    cmd.arg(dist_tag);

    let access = get_access(&pkg_json, config);
    if access == "public" {
      cmd.arg("--access");
      cmd.arg("public");
    }

    let output = cmd
      .output()
      .map_err(|e| OxrlsError::Bump(format!("Failed to run npm publish: {}", e)))?;

    if !output.status.success() {
      let stderr = String::from_utf8_lossy(&output.stderr);
      return Err(OxrlsError::Bump(format!(
        "Failed to publish \"{}\":\n{}",
        package_name, stderr
      )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let last_line = stdout.trim().lines().last().unwrap_or("ok");
    println!("  Published: {}", last_line);
    published += 1;
  }

  if dry_run {
    let total = manifest.packages.len();
    let would_publish = total - skipped as usize;
    println!(
      "\n[DRY RUN] {} package(s) would be published ({} skipped).",
      would_publish, skipped
    );
  } else {
    ReleaseManifest::remove(&workspace.root.join(".oxrls"))?;
    println!("\nDone! {} published, {} skipped.", published, skipped);
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::bump::PlannedBump;
  use crate::release_file::BumpType;
  use indexmap::IndexMap;

  #[test]
  fn test_manifest_roundtrip() {
    let tmp = tempfile::TempDir::new().unwrap();
    let release_dir = tmp.path().join(".oxrls");

    let mut bumps = IndexMap::new();
    bumps.insert(
      "@scope/core".to_string(),
      PlannedBump {
        package_name: "@scope/core".to_string(),
        old_version: semver::Version::new(1, 2, 3),
        new_version: semver::Version::new(1, 2, 4),
        bump_type: BumpType::Patch,
        summaries: vec![],
        release_files: vec![],
      },
    );

    let manifest = ReleaseManifest::from_bumps(&bumps);
    manifest.save(&release_dir).unwrap();

    let loaded = ReleaseManifest::load(&release_dir).unwrap();
    assert_eq!(loaded.packages, vec!["@scope/core"]);
  }

  #[test]
  fn test_manifest_sorting() {
    let mut bumps = IndexMap::new();
    bumps.insert(
      "@scope/zzz".to_string(),
      PlannedBump {
        package_name: "@scope/zzz".to_string(),
        old_version: semver::Version::new(1, 0, 0),
        new_version: semver::Version::new(1, 0, 1),
        bump_type: BumpType::Patch,
        summaries: vec![],
        release_files: vec![],
      },
    );
    bumps.insert(
      "@scope/aaa".to_string(),
      PlannedBump {
        package_name: "@scope/aaa".to_string(),
        old_version: semver::Version::new(1, 0, 0),
        new_version: semver::Version::new(1, 0, 1),
        bump_type: BumpType::Patch,
        summaries: vec![],
        release_files: vec![],
      },
    );

    let manifest = ReleaseManifest::from_bumps(&bumps);
    assert_eq!(manifest.packages, vec!["@scope/aaa", "@scope/zzz"]);
  }

  #[test]
  fn test_manifest_no_file_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let release_dir = tmp.path().join(".oxrls");
    let result = ReleaseManifest::load(&release_dir);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Run `oxrls bump` first"));
  }

  #[test]
  fn test_extract_pre_tag() {
    assert_eq!(extract_pre_tag("1.0.0-beta.1"), Some("beta".to_string()));
    assert_eq!(extract_pre_tag("1.0.0-rc.3"), Some("rc".to_string()));
    assert_eq!(extract_pre_tag("1.0.0"), None);
    assert_eq!(extract_pre_tag("1.0.0-alpha.1"), Some("alpha".to_string()));
  }
}
