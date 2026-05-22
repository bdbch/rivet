use crate::changelog::{generate_changelog_section, update_changelog, ChangelogEntry};
use crate::config::{InternalDepUpdate, OxrlsConfig};
use crate::error::{OxrlsError, Result};
use crate::package_json::PackageJson;
use crate::release_file::{consume_release_file, parse_release_file, BumpType, ReleaseFile};
use crate::version_bump::bump_version;
use crate::workspace::Workspace;
use indexmap::IndexMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// A planned version bump for a single package.
#[derive(Debug, Clone)]
pub struct PlannedBump {
  pub package_name: String,
  pub old_version: semver::Version,
  pub new_version: semver::Version,
  pub bump_type: BumpType,
  pub summaries: Vec<String>,
  /// The release files that caused this bump.
  pub release_files: Vec<PathBuf>,
}

/// A complete release plan built from pending release files.
#[derive(Debug, Clone)]
pub struct ReleasePlan {
  pub bumps: IndexMap<String, PlannedBump>,
  pub internal_dep_updates: Vec<InternalDepUpdateInfo>,
}

/// Information about an internal dependency range update.
#[derive(Debug, Clone)]
pub struct InternalDepUpdateInfo {
  pub dependent_package_path: PathBuf,
  pub dependent_package_name: String,
  pub dep_name: String,
  pub dep_type: String,
  pub old_range: String,
  pub new_range: String,
}

/// Build a release plan by reading all release files and computing bumps.
pub fn build_release_plan(
  workspace: &Workspace,
  config: &OxrlsConfig,
  release_dir: &Path,
) -> Result<ReleasePlan> {
  // Find all release files
  let release_files = find_release_files(release_dir)?;

  if release_files.is_empty() {
    return Err(OxrlsError::Bump(
      "No pending release files found.".to_string(),
    ));
  }

  // Parse all release files
  let parsed: Vec<ReleaseFile> = release_files
    .iter()
    .map(|path| parse_release_file(path))
    .collect::<Result<Vec<_>>>()?;

  // Validate all package references
  for rf in &parsed {
    for pkg_name in rf.releases.keys() {
      if !workspace.packages.contains_key(pkg_name) {
        return Err(OxrlsError::Bump(format!(
          "Release file {} references \"{}\", but no workspace package with that name exists.",
          rf.path.display(),
          pkg_name
        )));
      }
    }
  }

  // Merge bump types per package (highest priority wins)
  let mut merged_bumps: IndexMap<String, (BumpType, Vec<&ReleaseFile>)> = IndexMap::new();
  for rf in &parsed {
    for (pkg_name, bump_type) in &rf.releases {
      let entry = merged_bumps
        .entry(pkg_name.clone())
        .or_insert((*bump_type, vec![]));
      entry.0 = BumpType::max(Some(entry.0), *bump_type);
      entry.1.push(rf);
    }
  }

  // Compute new versions
  let mut bumps: IndexMap<String, PlannedBump> = IndexMap::new();

  for (pkg_name, (bump_type, refs)) in &merged_bumps {
    let pkg = workspace
      .packages
      .get(pkg_name)
      .ok_or_else(|| OxrlsError::Bump(format!("Package '{}' not found in workspace", pkg_name)))?;

    let old_version = pkg.package_json.semver_version()?;
    let new_version = bump_version(&old_version, *bump_type);

    // Collect summaries from the release files that reference this package
    let summaries: Vec<String> = refs.iter().map(|rf| rf.summary.clone()).collect();

    let release_files: Vec<PathBuf> = refs.iter().map(|rf| rf.path.clone()).collect();

    bumps.insert(
      pkg_name.clone(),
      PlannedBump {
        package_name: pkg_name.clone(),
        old_version,
        new_version,
        bump_type: *bump_type,
        summaries,
        release_files,
      },
    );
  }

  // Compute internal dependency updates
  let internal_updates = compute_internal_dep_updates(workspace, &bumps, config)?;

  Ok(ReleasePlan {
    bumps,
    internal_dep_updates: internal_updates,
  })
}

/// Compute which internal dependency ranges need updating.
fn compute_internal_dep_updates(
  workspace: &Workspace,
  bumps: &IndexMap<String, PlannedBump>,
  config: &OxrlsConfig,
) -> Result<Vec<InternalDepUpdateInfo>> {
  let mut updates = Vec::new();

  // Check all workspace packages for dependencies on bumped packages
  for (dep_name, pkg) in &workspace.packages {
    // If this package is itself bumped, no need to update self-dependency
    // Check all dependency fields
    let dep_fields: Vec<(&str, &Option<IndexMap<String, String>>)> = vec![
      ("dependencies", &pkg.package_json.dependencies),
      ("devDependencies", &pkg.package_json.dev_dependencies),
      ("peerDependencies", &pkg.package_json.peer_dependencies),
      (
        "optionalDependencies",
        &pkg.package_json.optional_dependencies,
      ),
    ];

    for (field_name, field) in &dep_fields {
      let deps = match field {
        Some(d) => d,
        None => continue,
      };

      for (dep_name_in_range, _range) in deps {
        // Check if this dependency is being bumped
        if let Some(bump) = bumps.get(dep_name_in_range) {
          // Check if we should update based on config
          if should_update_dependency(&config.update_internal_dependencies, &bump.bump_type) {
            // We'll compute the new range when applying
            updates.push(InternalDepUpdateInfo {
              dependent_package_path: pkg.dir.join("package.json"),
              dependent_package_name: dep_name.clone(),
              dep_name: dep_name_in_range.clone(),
              dep_type: field_name.to_string(),
              old_range: _range.clone(),
              new_range: String::new(), // filled in during apply
            });
          }
        }
      }
    }
  }

  Ok(updates)
}

/// Determine if we should update internal dependencies based on config.
fn should_update_dependency(config: &InternalDepUpdate, bump_type: &BumpType) -> bool {
  match config {
    InternalDepUpdate::Always => true,
    InternalDepUpdate::Never => false,
    InternalDepUpdate::Patch => true,
    InternalDepUpdate::Minor => bump_type.priority() >= BumpType::Minor.priority(),
    InternalDepUpdate::Major => bump_type.priority() >= BumpType::Major.priority(),
  }
}

/// Apply the release plan: write package.json files, update changelogs, remove release files.
/// If `dry_run` is true, only print what would happen without writing.
pub fn apply_release_plan(
  workspace: &Workspace,
  plan: &ReleasePlan,
  config: &OxrlsConfig,
  release_dir: &Path,
  dry_run: bool,
  archive: bool,
) -> Result<()> {
  if dry_run {
    print_plan(plan);
    return Ok(());
  }

  // Phase 1: Update package.json files
  for (_name, bump) in &plan.bumps {
    let pkg = workspace
      .packages
      .get(&bump.package_name)
      .ok_or_else(|| OxrlsError::Bump(format!("Package '{}' not found", bump.package_name)))?;

    let pkg_path = pkg.dir.join("package.json");
    let mut pkg_json = PackageJson::read(&pkg_path)?;
    pkg_json.set_version(&bump.new_version);
    PackageJson::write(&pkg_path, &pkg_json)?;

    println!(
      "  {} {} -> {}",
      bump.package_name, bump.old_version, bump.new_version
    );
  }

  // Phase 2: Update internal dependency ranges
  for update in &plan.internal_dep_updates {
    let mut pkg_json = PackageJson::read(&update.dependent_package_path)?;
    let field = match update.dep_type.as_str() {
      "dependencies" => &mut pkg_json.dependencies,
      "devDependencies" => &mut pkg_json.dev_dependencies,
      "peerDependencies" => &mut pkg_json.peer_dependencies,
      "optionalDependencies" => &mut pkg_json.optional_dependencies,
      _ => continue,
    };

    if let Some(ref mut deps) = field {
      if let Some(current_range) = deps.get(&update.dep_name).cloned() {
        // Find the old and new versions for this dependency
        if let Some(bump) = plan.bumps.get(&update.dep_name) {
          let new_range = crate::package_json::compute_new_range(
            &current_range,
            &bump.old_version,
            &bump.new_version,
          );
          if new_range != current_range {
            deps.insert(update.dep_name.clone(), new_range.clone());
            println!(
              "  {} {} ({}: {} -> {})",
              update.dependent_package_name,
              update.dep_name,
              update.dep_type,
              current_range,
              new_range
            );
          }
        }
      }
    }

    PackageJson::write(&update.dependent_package_path, &pkg_json)?;
  }

  // Phase 3: Update changelogs
  if config.changelog {
    for (_name, bump) in &plan.bumps {
      let pkg = workspace
        .packages
        .get(&bump.package_name)
        .ok_or_else(|| OxrlsError::Bump(format!("Package '{}' not found", bump.package_name)))?;

      let changelog_path = pkg.dir.join("CHANGELOG.md");

      // Group summaries by type for this package
      // Since all summaries come from release files that reference this package,
      // and we know the merged bump type, we associate all summaries with that type
      // Actually, we need to be smarter: a release file might reference multiple packages
      // with different bump types. We group by the bump type from each release file entry.
      let mut type_summaries: IndexMap<BumpType, Vec<String>> = IndexMap::new();
      for rf_path in &bump.release_files {
        // Re-parse to get the per-package bump type
        if let Ok(rf) = parse_release_file(rf_path) {
          if let Some(bt) = rf.releases.get(&bump.package_name) {
            type_summaries
              .entry(*bt)
              .or_default()
              .push(rf.summary.clone());
          }
        }
      }

      // If no type-specific summaries found, just use all summaries under the merged type
      if type_summaries.is_empty() {
        type_summaries.insert(bump.bump_type, bump.summaries.clone());
      }

      let entry = ChangelogEntry {
        package_name: bump.package_name.clone(),
        version: bump.new_version.to_string(),
        changes: type_summaries,
      };

      let section = generate_changelog_section(&entry);
      update_changelog(&changelog_path, &bump.package_name, &section)?;
    }
  }

  // Phase 4: Consume release files
  if archive {
    let archive_dir = release_dir.join("archive");
    for (_name, bump) in &plan.bumps {
      for rf_path in &bump.release_files {
        crate::release_file::archive_release_file(rf_path, &archive_dir)?;
        println!("  {} (archived)", rf_path.display());
      }
    }
  } else {
    let mut consumed: HashSet<PathBuf> = HashSet::new();
    for (_name, bump) in &plan.bumps {
      for rf_path in &bump.release_files {
        if consumed.insert(rf_path.clone()) {
          consume_release_file(rf_path)?;
          println!("  {}", rf_path.display());
        }
      }
    }
  }

  Ok(())
}

/// Print the release plan without making changes.
pub fn print_plan(plan: &ReleasePlan) {
  println!("Bumped packages:");
  for (_name, bump) in &plan.bumps {
    println!(
      "  {} {} -> {} ({})",
      bump.package_name,
      bump.old_version,
      bump.new_version,
      bump.bump_type_str()
    );
  }

  if !plan.internal_dep_updates.is_empty() {
    println!("\nUpdated internal dependencies:");
    for update in &plan.internal_dep_updates {
      println!(
        "  {} package.json\n    {} {} -> {}",
        update.dependent_package_name,
        update.dep_name,
        update.old_range,
        if update.new_range.is_empty() {
          "(computed during apply)".to_string()
        } else {
          update.new_range.clone()
        }
      );
    }
  }

  println!("\nConsumed release files:");
  let mut seen: HashSet<&PathBuf> = HashSet::new();
  for (_name, bump) in &plan.bumps {
    for rf_path in &bump.release_files {
      if seen.insert(rf_path) {
        println!("  {}", rf_path.display());
      }
    }
  }
}

/// Find all markdown files in the release directory.
pub fn find_release_files(release_dir: &Path) -> Result<Vec<PathBuf>> {
  if !release_dir.exists() {
    return Ok(vec![]);
  }

  let mut files = Vec::new();
  let entries = std::fs::read_dir(release_dir).map_err(OxrlsError::Io)?;

  for entry in entries {
    let entry = entry.map_err(OxrlsError::Io)?;
    let path = entry.path();
    if path.is_file() && path.extension().map(|e| e == "md").unwrap_or(false) {
      // Skip README.md
      if path.file_stem().map(|s| s == "README").unwrap_or(false) {
        continue;
      }
      files.push(path);
    }
  }

  files.sort();
  Ok(files)
}

impl PlannedBump {
  fn bump_type_str(&self) -> &str {
    match self.bump_type {
      BumpType::Patch => "patch",
      BumpType::Minor => "minor",
      BumpType::Major => "major",
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::workspace::load_workspace;
  use tempfile::TempDir;

  fn create_test_workspace(tmp: &TempDir) -> Workspace {
    // Root package.json
    let root_pkg = serde_json::json!({
        "name": "root",
        "version": "1.0.0",
        "workspaces": ["packages/*"]
    });
    std::fs::create_dir_all(tmp.path().join("packages/core")).unwrap();
    std::fs::create_dir_all(tmp.path().join("packages/react")).unwrap();
    std::fs::write(
      tmp.path().join("package.json"),
      serde_json::to_string_pretty(&root_pkg).unwrap(),
    )
    .unwrap();

    // Core package
    let core_pkg = serde_json::json!({
        "name": "@scope/core",
        "version": "1.2.3"
    });
    std::fs::write(
      tmp.path().join("packages/core/package.json"),
      serde_json::to_string_pretty(&core_pkg).unwrap(),
    )
    .unwrap();

    // React package with dependency
    let react_pkg = serde_json::json!({
        "name": "@scope/react",
        "version": "1.0.0",
        "dependencies": {
            "@scope/core": "^1.2.3"
        }
    });
    std::fs::write(
      tmp.path().join("packages/react/package.json"),
      serde_json::to_string_pretty(&react_pkg).unwrap(),
    )
    .unwrap();

    load_workspace(tmp.path()).unwrap()
  }

  #[test]
  fn test_build_release_plan() {
    let tmp = TempDir::new().unwrap();
    let workspace = create_test_workspace(&tmp);

    // Create release dir and file
    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    let content = r#"---
"@scope/core": patch
---

Fix transaction mapping bug."#;
    std::fs::write(release_dir.join("calm-blue-fox.md"), content).unwrap();

    let config = OxrlsConfig::default();
    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

    assert_eq!(plan.bumps.len(), 1);
    let bump = plan.bumps.get("@scope/core").unwrap();
    assert_eq!(bump.old_version, semver::Version::new(1, 2, 3));
    assert_eq!(bump.new_version, semver::Version::new(1, 2, 4));
    assert_eq!(bump.bump_type, BumpType::Patch);
  }

  #[test]
  fn test_build_plan_missing_package() {
    let tmp = TempDir::new().unwrap();
    let workspace = create_test_workspace(&tmp);

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    let content = r#"---
"@scope/missing": patch
---

Fix something."#;
    std::fs::write(release_dir.join("bad.md"), content).unwrap();

    let config = OxrlsConfig::default();
    let result = build_release_plan(&workspace, &config, &release_dir);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("@scope/missing"));
  }

  #[test]
  fn test_build_plan_bump_precedence() {
    let tmp = TempDir::new().unwrap();
    let workspace = create_test_workspace(&tmp);

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    // Two release files with different bump types for same package
    let content1 = r#"---
"@scope/core": patch
---

Fix bug."#;
    std::fs::write(release_dir.join("file1.md"), content1).unwrap();

    let content2 = r#"---
"@scope/core": minor
---

Add feature."#;
    std::fs::write(release_dir.join("file2.md"), content2).unwrap();

    let config = OxrlsConfig::default();
    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

    let bump = plan.bumps.get("@scope/core").unwrap();
    assert_eq!(bump.new_version, semver::Version::new(1, 3, 0)); // minor wins over patch
  }

  #[test]
  fn test_apply_release_plan_dry_run() {
    let tmp = TempDir::new().unwrap();
    let workspace = create_test_workspace(&tmp);

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    let content = r#"---
"@scope/core": patch
---

Fix bug."#;
    std::fs::write(release_dir.join("test.md"), content).unwrap();

    let config = OxrlsConfig::default();
    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

    // Dry run should not modify files
    apply_release_plan(&workspace, &plan, &config, &release_dir, true, false).unwrap();

    // Check version unchanged
    let core_pkg = PackageJson::read(&tmp.path().join("packages/core/package.json")).unwrap();
    assert_eq!(core_pkg.version.as_deref(), Some("1.2.3"));
  }

  #[test]
  fn test_internal_dependency_updates() {
    let tmp = TempDir::new().unwrap();
    let workspace = create_test_workspace(&tmp);

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    let content = r#"---
"@scope/core": patch
---

Fix bug."#;
    std::fs::write(release_dir.join("test.md"), content).unwrap();

    let config = OxrlsConfig::default();
    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

    // Check that @scope/react's dependency on @scope/core is flagged for update
    let has_core_update = plan
      .internal_dep_updates
      .iter()
      .any(|u| u.dep_name == "@scope/core" && u.dependent_package_name == "@scope/react");
    assert!(has_core_update);
  }
}
