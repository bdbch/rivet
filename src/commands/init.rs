//! `oxrls init` — initialize oxrls configuration in a workspace.
//!
//! This command creates a config file (`.oxrls/config.json`) and a release
//! directory. It also runs an interactive wizard in non-`--force` mode.

use std::path::Path;

use glob::Pattern;

use crate::config::OxrlsConfig;
use crate::error::{OxrlsError, Result};
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
        .map_err(|e| OxrlsError::Config(format!("Invalid glob pattern: {}", e)))?;
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
        return Err(OxrlsError::Config(format!(
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
    return Err(OxrlsError::Config(
      "No packages matched the given patterns.".to_string(),
    ));
  }
  Ok(matched)
}

pub fn cmd_init(force: bool, release_dir: Option<&str>, non_interactive: bool) -> Result<()> {
  let cwd = std::env::current_dir().map_err(OxrlsError::Io)?;

  let root = find_workspace_root(Path::new(".")).map_err(|e| {
    OxrlsError::Config(format!(
      "No workspace found: {}. Run from a repo with a package.json.",
      e
    ))
  })?;
  let workspace = load_workspace(&root)?;
  let is_monorepo = workspace.packages.len() > 1;

  let config_path = cwd.join(".oxrls").join("config.json");

  let mut config = OxrlsConfig::default();

  if let Some(dir) = release_dir {
    config.release_dir = dir.to_string();
  }

  if !non_interactive {
    run_init_wizard(&mut config, &workspace, is_monorepo)?;
  }

  OxrlsConfig::write_to(&config_path, &config, force)?;
  println!("Created config file: {}", config_path.display());

  let release_dir = cwd.join(&config.release_dir);
  std::fs::create_dir_all(&release_dir)
    .map_err(|e| OxrlsError::Config(format!("Failed to create release dir: {}", e)))?;

  let readme_path = release_dir.join("README.md");
  if !readme_path.exists() {
    let readme_content = format!(
      "# {} Release Files\n\nThis directory contains pending release files.\n\
            Use `oxrls new` to create a release file and `oxrls bump` to apply them.\n",
      config.release_dir.trim_start_matches('.')
    );
    std::fs::write(&readme_path, readme_content)
      .map_err(|e| OxrlsError::Config(format!("Failed to create README: {}", e)))?;
    println!("Created release directory: {}", release_dir.display());
  }

  println!("\noxrls is ready! Use `oxrls new` to create a release file.");
  Ok(())
}
