use crate::error::{Result, RivetError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The pre-release state persisted in `.rivet/pre.json`.
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
        .map_err(|e| RivetError::Config(format!("Failed to read pre.json: {}", e)))?;
      Ok(serde_json::from_str(&content)?)
    } else {
      Ok(PreState::default())
    }
  }

  /// Save pre-state to the release directory.
  pub fn save(&self, release_dir: &Path) -> Result<()> {
    let path = Self::path(release_dir);
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent).map_err(RivetError::Io)?;
    }
    let content = serde_json::to_string_pretty(self)?;
    std::fs::write(&path, content).map_err(RivetError::Io)?;
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
