use crate::error::{OxrlsError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The update-internal-dependencies strategy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum InternalDepUpdate {
  Always,
  #[default]
  Patch,
  Minor,
  Major,
  Never,
}

impl InternalDepUpdate {
  pub fn bump_priority(&self) -> u8 {
    match self {
      InternalDepUpdate::Never => 0,
      InternalDepUpdate::Major => 4,
      InternalDepUpdate::Minor => 3,
      InternalDepUpdate::Patch => 2,
      InternalDepUpdate::Always => 1,
    }
  }
}

/// The access level for publishing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Access {
  #[default]
  Public,
  Restricted,
}

/// Schema for oxrls.json config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OxrlsConfig {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub schema: Option<String>,

  #[serde(default = "default_release_dir")]
  pub release_dir: String,

  #[serde(default = "default_changelog")]
  pub changelog: bool,

  #[serde(default = "default_update_internal_deps")]
  pub update_internal_dependencies: InternalDepUpdate,

  #[serde(default = "default_base_branch")]
  pub base_branch: String,

  #[serde(default)]
  pub access: Access,
}

fn default_release_dir() -> String {
  ".oxrls".to_string()
}

fn default_changelog() -> bool {
  true
}

fn default_update_internal_deps() -> InternalDepUpdate {
  InternalDepUpdate::Patch
}

fn default_base_branch() -> String {
  "main".to_string()
}

impl Default for OxrlsConfig {
  fn default() -> Self {
    Self {
      schema: Some("https://oxrelease.dev/schema.json".to_string()),
      release_dir: default_release_dir(),
      changelog: default_changelog(),
      update_internal_dependencies: default_update_internal_deps(),
      base_branch: default_base_branch(),
      access: Access::Public,
    }
  }
}

const CONFIG_FILE_NAMES: &[&str] = &["oxrls.json", ".oxrls.json"];

impl OxrlsConfig {
  /// Find and load config from the given directory or one of its parents.
  pub fn load(start_dir: &Path) -> Result<(Self, PathBuf)> {
    let cwd = std::env::current_dir().map_err(OxrlsError::Io)?;
    let search_start = if start_dir.is_absolute() {
      start_dir.to_path_buf()
    } else {
      cwd.join(start_dir)
    };

    // Walk up the directory tree
    let mut current = Some(search_start.as_path());
    while let Some(dir) = current {
      for name in CONFIG_FILE_NAMES {
        let path = dir.join(name);
        if path.exists() {
          let content = std::fs::read_to_string(&path).map_err(OxrlsError::Io)?;
          let config: OxrlsConfig = serde_json::from_str(&content)?;
          return Ok((config, path));
        }
      }
      current = dir.parent();
    }

    // No config file found — return defaults
    let config = OxrlsConfig::default();
    Ok((config, PathBuf::new()))
  }

  /// Find the config dir (the release_dir relative to config file location or cwd).
  pub fn release_dir_abs(&self, config_path: &Path) -> PathBuf {
    let base = config_path.parent().unwrap_or_else(|| Path::new("."));
    base.join(&self.release_dir)
  }

  /// Write config to a file.
  pub fn write_to(path: &Path, config: &Self, force: bool) -> Result<()> {
    if path.exists() && !force {
      return Err(OxrlsError::Config(format!(
        "Config file already exists at {}; use --force to overwrite",
        path.display()
      )));
    }
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent).map_err(OxrlsError::Io)?;
    }
    let content = serde_json::to_string_pretty(config)?;
    std::fs::write(path, content).map_err(OxrlsError::Io)?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;

  #[test]
  fn test_default_config() {
    let config = OxrlsConfig::default();
    assert_eq!(config.release_dir, ".oxrls");
    assert!(config.changelog);
    assert_eq!(config.base_branch, "main");
    assert_eq!(config.access, Access::Public);
  }

  #[test]
  fn test_load_no_config_returns_defaults() {
    let tmp = TempDir::new().unwrap();
    let (config, path) = OxrlsConfig::load(tmp.path()).unwrap();
    assert_eq!(config.release_dir, ".oxrls");
    assert!(path.as_os_str().is_empty());
  }

  #[test]
  fn test_write_and_load_config() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("oxrls.json");
    let config = OxrlsConfig {
      release_dir: ".myrelease".to_string(),
      ..Default::default()
    };
    OxrlsConfig::write_to(&config_path, &config, false).unwrap();

    let (loaded, _) = OxrlsConfig::load(tmp.path()).unwrap();
    assert_eq!(loaded.release_dir, ".myrelease");
  }
}
