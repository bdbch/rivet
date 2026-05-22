use crate::error::{OxrlsError, Result};
use crate::package_json::PackageJson;
use glob::glob;
use indexmap::IndexMap;
use std::path::{Path, PathBuf};

/// A discovered workspace package.
#[derive(Debug, Clone)]
pub struct WorkspacePackage {
  /// The absolute directory of the package.
  pub dir: PathBuf,
  /// The parsed package.json.
  pub package_json: PackageJson,
}

/// Information about the workspace root.
#[derive(Debug, Clone)]
pub struct Workspace {
  /// The root directory of the workspace.
  pub root: PathBuf,
  /// The root package.json.
  pub root_package_json: PackageJson,
  /// All discovered workspace packages, keyed by package name.
  pub packages: IndexMap<String, WorkspacePackage>,
}

/// Detect the workspace root by looking for a package.json with workspaces field,
/// a pnpm-workspace.yaml, or — failing those — the nearest ancestor with a package.json.
pub fn find_workspace_root(start_dir: &Path) -> Result<PathBuf> {
  let cwd = if start_dir.is_relative() {
    std::env::current_dir()
      .map_err(OxrlsError::Io)?
      .join(start_dir)
  } else {
    start_dir.to_path_buf()
  };

  let mut current = Some(cwd.as_path());
  while let Some(dir) = current {
    let pkg_path = dir.join("package.json");
    if pkg_path.exists()
      && let Ok(content) = std::fs::read_to_string(&pkg_path)
      && let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content)
      && pkg.get("workspaces").is_some()
    {
      return Ok(dir.to_path_buf());
    }
    // No workspaces field — a single-package repo.
    // We don't return immediately because there might be a workspace
    // marker in a parent directory.
    // Also check for pnpm-workspace.yaml
    let pnpm_yaml = dir.join("pnpm-workspace.yaml");
    if pnpm_yaml.exists() {
      return Ok(dir.to_path_buf());
    }
    // Also check for lerna.json
    let lerna_json = dir.join("lerna.json");
    if lerna_json.exists() {
      return Ok(dir.to_path_buf());
    }
    current = dir.parent();
  }

  // Fallback: walk up again and return the nearest ancestor with a package.json
  let mut current = Some(cwd.as_path());
  let mut last_with_package_json: Option<PathBuf> = None;
  while let Some(dir) = current {
    let pkg_path = dir.join("package.json");
    if pkg_path.exists() {
      last_with_package_json = Some(dir.to_path_buf());
    }
    current = dir.parent();
  }

  last_with_package_json.ok_or_else(|| {
    OxrlsError::Workspace(
      "No package.json found in any parent directory. Are you in a Node.js project?".to_string(),
    )
  })
}

/// Load the complete workspace.
pub fn load_workspace(root: &Path) -> Result<Workspace> {
  let root_pkg_path = root.join("package.json");
  let root_pkg_content = std::fs::read_to_string(&root_pkg_path)
    .map_err(|e| OxrlsError::Workspace(format!("Failed to read root package.json: {}", e)))?;
  let root_package_json: PackageJson = serde_json::from_str(&root_pkg_content)?;

  // Collect workspace glob patterns
  let patterns = get_workspace_globs(root)?;

  let mut packages: IndexMap<String, WorkspacePackage> = IndexMap::new();

  if patterns.is_empty() {
    // No workspace config found — treat the root as a single package
    let root_name = root_package_json.name.clone();
    if let Some(name) = root_name {
      let wp = WorkspacePackage {
        dir: root.to_path_buf(),
        package_json: root_package_json.clone(),
      };
      packages.insert(name.clone(), wp);
    }
  } else {
    for pattern in &patterns {
      let full_pattern = root.join(pattern).to_string_lossy().to_string();
      // Determine the package.json glob
      let pkg_json_pattern = if full_pattern.ends_with('/') || full_pattern.ends_with("\\") {
        format!("{}package.json", full_pattern)
      } else if full_pattern.ends_with("package.json") {
        full_pattern.clone()
      } else {
        // It's a directory glob, add package.json
        let trimmed = full_pattern.trim_end_matches('/');
        format!("{}/package.json", trimmed)
      };

      // Use glob to find all matching package.json files
      if let Ok(entries) = glob(&pkg_json_pattern) {
        for entry in entries.flatten() {
          // Skip root package.json
          if entry.parent() == Some(root) {
            continue;
          }
          if let Ok(content) = std::fs::read_to_string(&entry)
            && let Ok(pkg) = serde_json::from_str::<PackageJson>(&content)
          {
            let name = pkg.name.clone();
            if let Some(name) = name {
              let dir = entry.parent().unwrap_or(&entry).to_path_buf();
              let wp = WorkspacePackage {
                dir,
                package_json: pkg,
              };
              packages.entry(name.clone()).or_insert(wp);
            }
          }
        }
      }
    }
  }

  Ok(Workspace {
    root: root.to_path_buf(),
    root_package_json,
    packages,
  })
}

/// Extract workspace glob patterns from package.json or pnpm-workspace.yaml.
fn get_workspace_globs(root: &Path) -> Result<Vec<String>> {
  // Check pnpm-workspace.yaml first
  let pnpm_yaml = root.join("pnpm-workspace.yaml");
  if pnpm_yaml.exists()
    && let Ok(content) = std::fs::read_to_string(&pnpm_yaml)
    && let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(&content)
    && let Some(packages) = yaml.get("packages").and_then(|v| v.as_sequence())
  {
    let globs: Vec<String> = packages
      .iter()
      .filter_map(|v| v.as_str().map(|s| s.to_string()))
      .collect();
    if !globs.is_empty() {
      return Ok(globs);
    }
  }

  // Check package.json workspaces
  let root_pkg_path = root.join("package.json");
  if root_pkg_path.exists()
    && let Ok(content) = std::fs::read_to_string(&root_pkg_path)
    && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
    && let Some(workspaces) = json.get("workspaces")
  {
    // Array format: ["packages/*", "apps/*"]
    if let Some(arr) = workspaces.as_array() {
      return Ok(
        arr
          .iter()
          .filter_map(|v| v.as_str().map(|s| s.to_string()))
          .collect(),
      );
    }
    // Object format: { "packages": ["packages/*"], "nohoist": [...] }
    if let Some(obj) = workspaces.as_object()
      && let Some(pkg_arr) = obj.get("packages").and_then(|v| v.as_array())
    {
      return Ok(
        pkg_arr
          .iter()
          .filter_map(|v| v.as_str().map(|s| s.to_string()))
          .collect(),
      );
    }
  }

  Ok(vec![])
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;

  fn create_package_json(dir: &Path, name: &str, version: &str, workspaces: Option<&[&str]>) {
    let mut map = serde_json::Map::new();
    map.insert(
      "name".to_string(),
      serde_json::Value::String(name.to_string()),
    );
    map.insert(
      "version".to_string(),
      serde_json::Value::String(version.to_string()),
    );
    if let Some(ws) = workspaces {
      let arr: Vec<serde_json::Value> = ws
        .iter()
        .map(|s| serde_json::Value::String(s.to_string()))
        .collect();
      map.insert("workspaces".to_string(), serde_json::Value::Array(arr));
    }
    std::fs::create_dir_all(dir).unwrap();
    let content = serde_json::to_string_pretty(&map).unwrap();
    std::fs::write(dir.join("package.json"), content).unwrap();
  }

  #[test]
  fn test_find_workspace_root() {
    let tmp = TempDir::new().unwrap();
    create_package_json(tmp.path(), "root", "1.0.0", Some(&["packages/*"]));
    let inner = tmp.path().join("packages").join("foo");
    std::fs::create_dir_all(&inner).unwrap();

    let root = find_workspace_root(&inner).unwrap();
    assert_eq!(root, tmp.path());
  }

  #[test]
  fn test_load_workspace_with_packages() {
    let tmp = TempDir::new().unwrap();
    create_package_json(tmp.path(), "root", "1.0.0", Some(&["packages/*"]));

    // Create a workspace package
    let pkg_dir = tmp.path().join("packages").join("core");
    std::fs::create_dir_all(&pkg_dir).unwrap();
    let pkg_json = serde_json::json!({
        "name": "@scope/core",
        "version": "1.2.3",
        "dependencies": {
            "something": "^1.0.0"
        }
    });
    std::fs::write(
      pkg_dir.join("package.json"),
      serde_json::to_string_pretty(&pkg_json).unwrap(),
    )
    .unwrap();

    let workspace = load_workspace(tmp.path()).unwrap();
    assert!(workspace.packages.contains_key("@scope/core"));
    assert_eq!(
      workspace.packages["@scope/core"]
        .package_json
        .version
        .as_deref(),
      Some("1.2.3")
    );
  }

  #[test]
  fn test_load_single_package_no_workspace_config() {
    let tmp = TempDir::new().unwrap();

    // Root package.json — no workspaces field
    let root_pkg = serde_json::json!({
        "name": "my-single-pkg",
        "version": "0.1.0",
        "dependencies": {
            "lodash": "^4.17.0"
        }
    });
    std::fs::write(
      tmp.path().join("package.json"),
      serde_json::to_string_pretty(&root_pkg).unwrap(),
    )
    .unwrap();

    let workspace = load_workspace(tmp.path()).unwrap();

    // Should treat root as the only package
    assert_eq!(workspace.packages.len(), 1);
    assert!(workspace.packages.contains_key("my-single-pkg"));
    assert_eq!(
      workspace.packages["my-single-pkg"]
        .package_json
        .version
        .as_deref(),
      Some("0.1.0")
    );
    assert_eq!(workspace.packages["my-single-pkg"].dir, tmp.path());
  }

  #[test]
  fn test_find_root_single_package_no_workspace_config() {
    let tmp = TempDir::new().unwrap();

    let root_pkg = serde_json::json!({
        "name": "my-app",
        "version": "1.0.0"
    });
    std::fs::write(
      tmp.path().join("package.json"),
      serde_json::to_string_pretty(&root_pkg).unwrap(),
    )
    .unwrap();

    // Finding root from a subdirectory should walk up to the root
    let subdir = tmp.path().join("src").join("lib");
    std::fs::create_dir_all(&subdir).unwrap();

    let root = find_workspace_root(&subdir).unwrap();
    assert_eq!(root, tmp.path());
  }
}
