use crate::config::OxrlsConfig;
use crate::error::{OxrlsError, Result};
use crate::workspace::Workspace;
use glob::Pattern;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The pre-release state persisted in `.oxrls/pre.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PreState {
  /// Map of package name → (tag, current counter).
  /// The counter tracks how many pre-releases have been issued for this
  /// package+tag combination. Starts at 1, increments on each bump.
  #[serde(default)]
  pub pre_versions: BTreeMap<String, PreVersionEntry>,
}

/// Single entry tracking a package's pre-release counter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreVersionEntry {
  pub tag: String,
  pub count: u64,
}

impl PreState {
  /// Path to the pre.json file within the release directory.
  pub fn path(release_dir: &Path) -> PathBuf {
    release_dir.join("pre.json")
  }

  /// Load pre-state from the release directory.
  pub fn load(release_dir: &Path) -> Result<Self> {
    let path = Self::path(release_dir);
    if path.exists() {
      let content = std::fs::read_to_string(&path)
        .map_err(|e| OxrlsError::Config(format!("Failed to read pre.json: {}", e)))?;
      Ok(serde_json::from_str(&content)?)
    } else {
      Ok(PreState::default())
    }
  }

  /// Save pre-state to the release directory.
  pub fn save(&self, release_dir: &Path) -> Result<()> {
    let path = Self::path(release_dir);
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent).map_err(OxrlsError::Io)?;
    }
    let content = serde_json::to_string_pretty(self)?;
    std::fs::write(&path, content).map_err(OxrlsError::Io)?;
    Ok(())
  }

  /// Get the current counter for a package+tag pair (0 if not yet started).
  pub fn get_count(&self, package_name: &str, tag: &str) -> u64 {
    self
      .pre_versions
      .get(package_name)
      .filter(|e| e.tag == tag)
      .map(|e| e.count)
      .unwrap_or(0)
  }

  /// Increment the counter for a package+tag pair. Starts at 1 on first call.
  pub fn increment(&mut self, package_name: &str, tag: &str) -> u64 {
    let new_count = self.get_count(package_name, tag) + 1;
    self.pre_versions.insert(
      package_name.to_string(),
      PreVersionEntry {
        tag: tag.to_string(),
        count: new_count,
      },
    );
    new_count
  }

  /// Remove a package from pre-release tracking (when exiting pre-mode).
  pub fn remove(&mut self, package_name: &str) {
    self.pre_versions.remove(package_name);
  }

  /// Check if a package is currently in pre-release mode.
  pub fn is_in_pre(&self, package_name: &str) -> bool {
    self.pre_versions.contains_key(package_name)
  }
}

/// Determine the pre-release tag and counter for a package, if it's in pre-mode.
/// Returns `Some((tag, counter))` if the package should produce a pre-release version.
/// The counter is the *next* value to use (already incremented in the returned state).
pub fn resolve_pre_release(
  package_name: &str,
  config: &OxrlsConfig,
  pre_state: &mut PreState,
  _workspace: &Workspace,
) -> Option<(String, u64)> {
  // Check the package against each preMode entry
  for entry in &config.pre_mode {
    let matches = entry.packages.iter().any(|pattern| {
      if let Ok(pat) = Pattern::new(pattern) {
        pat.matches(package_name)
      } else {
        package_name == pattern
      }
    });

    if matches {
      // Increment and get the new counter value
      let count = pre_state.increment(package_name, &entry.tag);
      return Some((entry.tag.clone(), count));
    }
  }

  None
}

/// Check whether a pre-release version string matches a tag and counter.
/// Used to detect if a package is already in pre-release state.
pub fn version_is_pre_release(version: &str) -> Option<(String, u64)> {
  let v = semver::Version::parse(version).ok()?;
  if v.pre.is_empty() {
    return None;
  }
  // Parse the pre-release identifiers
  let parts: Vec<&str> = v.pre.split('.').collect();
  if parts.len() == 2 {
    let tag = parts[0].to_string();
    let count = parts[1].parse::<u64>().ok()?;
    Some((tag, count))
  } else {
    // Just return the tag with counter 0 if we can't parse it
    Some((v.pre.to_string(), 0))
  }
}

/// Apply a pre-release tag and counter to a base version string.
pub fn apply_pre_release(base_version: &semver::Version, tag: &str, count: u64) -> semver::Version {
  let pre = format!("{}.{}", tag, count);
  semver::Version {
    major: base_version.major,
    minor: base_version.minor,
    patch: base_version.patch,
    pre: semver::Prerelease::new(&pre).unwrap_or_default(),
    build: Default::default(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::PreModeEntry;

  #[test]
  fn test_pre_state_increment() {
    let mut state = PreState::default();
    assert_eq!(state.get_count("@scope/core", "beta"), 0);

    let c1 = state.increment("@scope/core", "beta");
    assert_eq!(c1, 1);
    assert_eq!(state.get_count("@scope/core", "beta"), 1);

    let c2 = state.increment("@scope/core", "beta");
    assert_eq!(c2, 2);
  }

  #[test]
  fn test_pre_state_replacing_tag_resets_count() {
    let mut state = PreState::default();
    state.increment("@scope/pkg", "beta");
    state.increment("@scope/pkg", "beta");

    assert_eq!(state.get_count("@scope/pkg", "beta"), 2);

    // Moving to a new tag replaces the entry, resetting the counter
    state.increment("@scope/pkg", "alpha");
    assert_eq!(state.get_count("@scope/pkg", "alpha"), 1);
    assert_eq!(state.get_count("@scope/pkg", "beta"), 0);
  }

  #[test]
  fn test_resolve_pre_release() {
    let config = OxrlsConfig {
      pre_mode: vec![PreModeEntry {
        tag: "beta".to_string(),
        packages: vec!["@scope/core".to_string(), "@scope/react".to_string()],
      }],
      ..Default::default()
    };
    let mut pre_state = PreState::default();

    // Create a minimal workspace with the right package names
    let tmp = tempfile::TempDir::new().unwrap();
    let root_pkg =
      serde_json::json!({"name": "root", "version": "1.0.0", "workspaces": ["packages/*"]});
    std::fs::write(
      tmp.path().join("package.json"),
      serde_json::to_string_pretty(&root_pkg).unwrap(),
    )
    .unwrap();
    std::fs::create_dir_all(tmp.path().join("packages/core")).unwrap();
    std::fs::write(
      tmp.path().join("packages/core/package.json"),
      serde_json::to_string_pretty(&serde_json::json!({"name": "@scope/core", "version": "1.0.0"}))
        .unwrap(),
    )
    .unwrap();
    std::fs::create_dir_all(tmp.path().join("packages/react")).unwrap();
    std::fs::write(
      tmp.path().join("packages/react/package.json"),
      serde_json::to_string_pretty(
        &serde_json::json!({"name": "@scope/react", "version": "1.0.0"}),
      )
      .unwrap(),
    )
    .unwrap();
    let workspace = crate::workspace::load_workspace(tmp.path()).unwrap();

    let result = resolve_pre_release("@scope/core", &config, &mut pre_state, &workspace);
    assert_eq!(result, Some(("beta".to_string(), 1)));

    // Second call increments
    let result = resolve_pre_release("@scope/core", &config, &mut pre_state, &workspace);
    assert_eq!(result, Some(("beta".to_string(), 2)));

    // Package not in pre-mode
    let result = resolve_pre_release("@scope/other", &config, &mut pre_state, &workspace);
    assert_eq!(result, None);
  }

  #[test]
  fn test_resolve_with_glob() {
    let config = OxrlsConfig {
      pre_mode: vec![PreModeEntry {
        tag: "alpha".to_string(),
        packages: vec!["@scope/pre-*".to_string()],
      }],
      ..Default::default()
    };
    let mut pre_state = PreState::default();

    let tmp = tempfile::TempDir::new().unwrap();
    let root_pkg =
      serde_json::json!({"name": "root", "version": "1.0.0", "workspaces": ["packages/*"]});
    std::fs::write(
      tmp.path().join("package.json"),
      serde_json::to_string_pretty(&root_pkg).unwrap(),
    )
    .unwrap();
    std::fs::create_dir_all(tmp.path().join("packages/pre-alpha")).unwrap();
    std::fs::write(
      tmp.path().join("packages/pre-alpha/package.json"),
      serde_json::to_string_pretty(
        &serde_json::json!({"name": "@scope/pre-alpha", "version": "1.0.0"}),
      )
      .unwrap(),
    )
    .unwrap();
    let workspace = crate::workspace::load_workspace(tmp.path()).unwrap();

    let result = resolve_pre_release("@scope/pre-alpha", &config, &mut pre_state, &workspace);
    assert_eq!(result, Some(("alpha".to_string(), 1)));

    // Should not match
    let result = resolve_pre_release("@scope/other", &config, &mut pre_state, &workspace);
    assert_eq!(result, None);
  }

  #[test]
  fn test_apply_pre_release() {
    let base = semver::Version::new(2, 0, 0);
    let result = apply_pre_release(&base, "beta", 1);
    assert_eq!(result.to_string(), "2.0.0-beta.1");

    let result = apply_pre_release(&base, "beta", 3);
    assert_eq!(result.to_string(), "2.0.0-beta.3");

    let result = apply_pre_release(&base, "rc", 1);
    assert_eq!(result.to_string(), "2.0.0-rc.1");
  }

  #[test]
  fn test_pre_state_persistence() {
    let tmp = tempfile::TempDir::new().unwrap();
    let release_dir = tmp.path().join(".oxrls");

    let mut state = PreState::default();
    state.increment("@scope/core", "beta");
    state.increment("@scope/core", "beta");
    state.save(&release_dir).unwrap();

    let loaded = PreState::load(&release_dir).unwrap();
    assert_eq!(loaded.get_count("@scope/core", "beta"), 2);
  }

  #[test]
  fn test_pre_state_remove() {
    let mut state = PreState::default();
    state.increment("@scope/pkg", "beta");
    assert!(state.is_in_pre("@scope/pkg"));

    state.remove("@scope/pkg");
    assert!(!state.is_in_pre("@scope/pkg"));
  }
}
