//! `rivet init` — initialize rivet configuration in a workspace.
//!
//! This command creates a config file (`.rivet/config.json`) and a release
//! directory. It also runs an interactive wizard in non-`--force` mode.

use std::path::Path;

use glob::Pattern;

use crate::config::{RivetConfig, find_existing_config};
use crate::error::{Result, RivetError};
use crate::init_wizard::run_init_wizard;
use crate::workspace::{Workspace, find_workspace_root, load_workspace};

/// Resolve package name patterns against the workspace, returning matched package names.
/// Supports glob patterns (`*`/`?`), exact package name matches, and fuzzy suffix matching.
pub(crate) fn resolve_package_patterns(
  patterns: &[String],
  workspace: &Workspace,
) -> Result<Vec<String>> {
  let mut matched: Vec<String> = Vec::new();
  for pattern in patterns {
    if pattern.contains('*') || pattern.contains('?') {
      let pat = Pattern::new(pattern)
        .map_err(|e| RivetError::Config(format!("Invalid glob pattern: {}", e)))?;
      for name in workspace.packages.keys() {
        if pat.matches(name) && !matched.contains(name) {
          matched.push(name.clone());
        }
      }
    } else if workspace.packages.contains_key(pattern) {
      if !matched.contains(pattern) {
        matched.push(pattern.clone());
      }
    } else {
      let matches: Vec<String> = workspace
        .packages
        .keys()
        .filter(|name| *name == pattern || name.ends_with(pattern.as_str()))
        .cloned()
        .collect();
      if matches.is_empty() {
        return Err(RivetError::Config(format!(
          "No workspace package matches \"{}\". Available packages:\n  {}",
          pattern,
          workspace
            .packages
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n  ")
        )));
      }
      for name in matches {
        if !matched.contains(&name) {
          matched.push(name);
        }
      }
    }
  }
  if matched.is_empty() {
    return Err(RivetError::Config(
      "No packages matched the given patterns.".to_string(),
    ));
  }
  Ok(matched)
}

pub fn cmd_init(force: bool, release_dir: Option<&str>, non_interactive: bool) -> Result<()> {
  let cwd = std::env::current_dir().map_err(RivetError::Io)?;
  cmd_init_at(&cwd, force, release_dir, non_interactive)
}

fn cmd_init_at(
  cwd: &Path,
  force: bool,
  release_dir: Option<&str>,
  non_interactive: bool,
) -> Result<()> {
  let root = find_workspace_root(cwd).map_err(|e| {
    RivetError::Config(format!(
      "No workspace found: {}. Run from a repo with a package.json.",
      e
    ))
  })?;
  let workspace = load_workspace(&root)?;
  let is_monorepo = workspace.packages.len() > 1;

  if !force && let Some(existing_path) = find_existing_config(cwd) {
    println!(
      "rivet is already initialized.\nConfig: {}\nEdit that file to change settings, or use --force to re-run the wizard.",
      existing_path.display()
    );
    return Ok(());
  }

  let config_path = cwd.join(".rivet").join("config.json");

  let mut config = RivetConfig::default();

  if let Some(dir) = release_dir {
    config.release_dir = dir.to_string();
  }

  if !non_interactive {
    run_init_wizard(&mut config, &workspace, is_monorepo)?;
  }

  RivetConfig::write_to(&config_path, &config, force)?;
  println!("Created config file: {}", config_path.display());

  let release_dir = cwd.join(&config.release_dir);
  std::fs::create_dir_all(&release_dir)
    .map_err(|e| RivetError::Config(format!("Failed to create release dir: {}", e)))?;

  let readme_path = release_dir.join("README.md");
  if !readme_path.exists() {
    let readme_content = format!(
      "# {} Release Files\n\nThis directory contains pending release files.\n\
            Use `rivet new` to create a release file and `rivet bump` to apply them.\n",
      config.release_dir.trim_start_matches('.')
    );
    std::fs::write(&readme_path, readme_content)
      .map_err(|e| RivetError::Config(format!("Failed to create README: {}", e)))?;
    println!("Created release directory: {}", release_dir.display());
  }

  println!("\nrivet is ready! Use `rivet new` to create a release file.");
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;
  use tempfile::TempDir;

  fn write_package_json(dir: &TempDir) {
    let pkg = r#"{"name": "test", "version": "1.0.0"}"#;
    fs::write(dir.path().join("package.json"), pkg).unwrap();
  }

  #[test]
  fn test_init_returns_early_when_config_exists() {
    let dir = TempDir::new().unwrap();
    write_package_json(&dir);

    let config_dir = dir.path().join(".rivet");
    fs::create_dir_all(&config_dir).unwrap();
    let config_content = r#"{"release_dir": ".release", "version": "1.0.0"}"#;
    fs::write(config_dir.join("config.json"), config_content).unwrap();

    let result = cmd_init_at(dir.path(), false, None, true);
    assert!(result.is_ok());

    let actual = fs::read_to_string(config_dir.join("config.json")).unwrap();
    assert_eq!(actual, config_content);
  }

  #[test]
  fn test_init_force_overwrites_existing_config() {
    let dir = TempDir::new().unwrap();
    write_package_json(&dir);

    let config_dir = dir.path().join(".rivet");
    fs::create_dir_all(&config_dir).unwrap();
    let custom_config = r#"{"release_dir": ".custom-release", "version": "1.0.0"}"#;
    fs::write(config_dir.join("config.json"), custom_config).unwrap();

    let result = cmd_init_at(dir.path(), true, None, true);
    assert!(result.is_ok());

    let actual = fs::read_to_string(config_dir.join("config.json")).unwrap();
    assert!(!actual.contains(".custom-release"));
  }

  #[test]
  fn test_init_creates_config_when_none_exists() {
    let dir = TempDir::new().unwrap();
    write_package_json(&dir);

    let result = cmd_init_at(dir.path(), false, None, true);
    assert!(result.is_ok());

    let config_path = dir.path().join(".rivet").join("config.json");
    assert!(config_path.exists());

    let release_dir = dir.path().join(".rivet");
    assert!(release_dir.exists());
  }

  #[test]
  fn test_find_existing_config_finds_all_three_names() {
    let cases: [(&str, fn(&std::path::Path) -> std::path::PathBuf); 3] = [
      (".rivet/config.json", |p| {
        let d = p.join(".rivet");
        let _ = fs::create_dir_all(&d);
        fs::write(d.join("config.json"), "{}").unwrap();
        p.join(".rivet").join("config.json")
      }),
      ("rivet.json", |p| {
        fs::write(p.join("rivet.json"), "{}").unwrap();
        p.join("rivet.json")
      }),
      (".rivet.json", |p| {
        fs::write(p.join(".rivet.json"), "{}").unwrap();
        p.join(".rivet.json")
      }),
    ];

    for (name, setup) in &cases {
      let dir = TempDir::new().unwrap();
      let expected = setup(dir.path());
      let found = find_existing_config(dir.path());
      assert!(found.is_some(), "Expected to find config named {}", name);
      assert_eq!(found.unwrap(), expected);
    }
  }

  #[test]
  fn test_find_existing_config_returns_none_when_no_config() {
    let dir = TempDir::new().unwrap();
    let found = find_existing_config(dir.path());
    assert!(found.is_none());
  }

  #[test]
  fn test_find_existing_config_searches_parent_dirs() {
    let dir = TempDir::new().unwrap();
    let child = dir.path().join("subdir");
    fs::create_dir_all(&child).unwrap();

    let config_dir = dir.path().join(".rivet");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.json"), "{}").unwrap();

    let found = find_existing_config(&child);
    assert!(found.is_some());
  }

  #[test]
  fn test_init_non_interactive_returns_early_when_config_exists() {
    let dir = TempDir::new().unwrap();
    write_package_json(&dir);

    let config_dir = dir.path().join(".rivet");
    fs::create_dir_all(&config_dir).unwrap();
    let config_content = r#"{"release_dir": ".release", "version": "1.0.0"}"#;
    fs::write(config_dir.join("config.json"), config_content).unwrap();

    let result = cmd_init_at(dir.path(), false, None, true);
    assert!(result.is_ok());

    let actual = fs::read_to_string(config_dir.join("config.json")).unwrap();
    assert_eq!(actual, config_content);
  }
}
