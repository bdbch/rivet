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

impl std::fmt::Display for Access {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Access::Public => write!(f, "public"),
      Access::Restricted => write!(f, "restricted"),
    }
  }
}

/// Schema for oxrls.json config.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OxrlsConfig {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub schema: Option<String>,

  #[serde(default = "default_release_dir")]
  pub release_dir: String,

  /// Deprecated: use `generate_packages_changelog` instead.
  /// If set explicitly, it maps to `generate_packages_changelog`.
  #[serde(default = "default_changelog")]
  pub changelog: bool,

  /// Generate individual CHANGELOG.md files per workspace package.
  #[serde(default = "default_generate_packages_changelog")]
  pub generate_packages_changelog: bool,

  /// Generate a single global CHANGELOG.md in the project root.
  #[serde(default = "default_generate_global_changelog")]
  pub generate_global_changelog: bool,

  #[serde(default = "default_update_internal_deps")]
  pub update_internal_dependencies: InternalDepUpdate,

  #[serde(default = "default_base_branch")]
  pub base_branch: String,

  #[serde(default)]
  pub access: Access,

  /// When enabled, also bump the version in Cargo.toml files
  /// that are found alongside package.json.
  #[serde(default)]
  pub sync_cargo_toml: bool,

  /// Groups of packages that always share the same version.
  /// When any package in a fixed group is bumped, all packages
  /// in that group are bumped to the same new version.
  #[serde(default)]
  pub fixed: Vec<Vec<String>>,

  /// Groups of packages that share the same bump type.
  /// When a package in a linked group receives a bump, all
  /// packages in that group receive the same bump type.
  #[serde(default)]
  pub linked: Vec<Vec<String>>,

  /// Pre-release mode configuration.
  /// Packages listed here will produce pre-release versions (e.g., `2.0.0-beta.1`).
  #[serde(default)]
  pub pre_mode: Vec<PreModeEntry>,
}

/// A single pre-release mode entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreModeEntry {
  /// The pre-release tag (e.g., "beta", "alpha", "rc").
  pub tag: String,

  /// Package name patterns (supports glob and `!` negation, same as `fixed`/`linked`).
  pub packages: Vec<String>,
}

/// Information about which changelog mode(s) are active for a given run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChangelogMode {
  /// Generate per-package CHANGELOG.md files.
  pub per_package: bool,
  /// Generate a root-level CHANGELOG.md aggregating all changes.
  pub global: bool,
}

fn default_release_dir() -> String {
  ".oxrls".to_string()
}

fn default_changelog() -> bool {
  true
}

fn default_generate_packages_changelog() -> bool {
  true
}

fn default_generate_global_changelog() -> bool {
  false
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
      generate_packages_changelog: default_generate_packages_changelog(),
      generate_global_changelog: default_generate_global_changelog(),
      update_internal_dependencies: default_update_internal_deps(),
      base_branch: default_base_branch(),
      access: Access::Public,
      sync_cargo_toml: false,
      fixed: vec![],
      linked: vec![],
      pre_mode: vec![],
    }
  }
}

impl OxrlsConfig {
  /// Determine the effective changelog mode for the current run.
  ///
  /// Rules:
  /// - Legacy `changelog: false` disables all changelog generation.
  /// - In a solo repo (single package), `per_package` falls back to `global`.
  /// - If both are false, no changelog is generated.
  pub fn changelog_mode(&self, is_solo_repo: bool) -> ChangelogMode {
    // Legacy `changelog: false` disables changelogs entirely
    if !self.changelog {
      return ChangelogMode {
        per_package: false,
        global: false,
      };
    }

    let per_package = self.generate_packages_changelog;
    let global = self.generate_global_changelog;

    if is_solo_repo && per_package && !global {
      // Solo repo falls back to global
      ChangelogMode {
        per_package: false,
        global: true,
      }
    } else {
      ChangelogMode { per_package, global }
    }
  }
}

const CONFIG_FILE_NAMES: &[&str] = &[".oxrls/config.json", "oxrls.json", ".oxrls.json"];

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
  /// If the config file is already inside the release directory, returns the
  /// config's parent directory directly to avoid double-joining.
  pub fn release_dir_abs(&self, config_path: &Path) -> PathBuf {
    let base = config_path.parent().unwrap_or_else(|| Path::new("."));
    // If the config's parent directory already has the release_dir name, or if
    // the release_dir is ".", use the parent directly.
    if self.release_dir == "." || self.release_dir.is_empty()
      || base.file_name().and_then(|n| n.to_str()) == Some(self.release_dir.as_str())
    {
      base.to_path_buf()
    } else {
      base.join(&self.release_dir)
    }
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

  #[test]
  fn test_changelog_mode_default_monorepo() {
    let config = OxrlsConfig::default();
    let mode = config.changelog_mode(false);
    assert!(mode.per_package);
    assert!(!mode.global);
  }

  #[test]
  fn test_changelog_mode_solo_falls_back_to_global() {
    let config = OxrlsConfig::default();
    let mode = config.changelog_mode(true);
    // Solo repo: per_package falls back to global
    assert!(!mode.per_package);
    assert!(mode.global);
  }

  #[test]
  fn test_changelog_mode_explicit_both() {
    let config = OxrlsConfig {
      generate_packages_changelog: true,
      generate_global_changelog: true,
      ..Default::default()
    };
    let mode = config.changelog_mode(false);
    assert!(mode.per_package);
    assert!(mode.global);
  }

  #[test]
  fn test_changelog_mode_legacy_false_disables_all() {
    let config = OxrlsConfig {
      changelog: false,
      ..Default::default()
    };
    let mode = config.changelog_mode(false);
    assert!(!mode.per_package);
    assert!(!mode.global);
  }

  #[test]
  fn test_changelog_mode_both_false() {
    let config = OxrlsConfig {
      generate_packages_changelog: false,
      generate_global_changelog: false,
      changelog: true,
      ..Default::default()
    };
    let mode = config.changelog_mode(false);
    assert!(!mode.per_package);
    assert!(!mode.global);
  }

  #[test]
  fn test_changelog_mode_solo_global_only() {
    let config = OxrlsConfig {
      generate_packages_changelog: false,
      generate_global_changelog: true,
      ..Default::default()
    };
    let mode = config.changelog_mode(true);
    assert!(!mode.per_package);
    assert!(mode.global);
  }
}
