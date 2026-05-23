//! `oxrls pre` — pre-release mode management.
//!
//! Commands for entering, exiting, and inspecting pre-release mode.
//! Pre-release mode applies pre-release version suffixes (e.g. `-beta.1`)
//! to version bumps for packages configured to a given tag.

use std::path::Path;

use inquire::{MultiSelect, Text};

use crate::config::{OxrlsConfig, PreModeEntry};
use crate::error::{OxrlsError, Result};
use crate::premode::PreState;
use crate::workspace::{find_workspace_root, load_workspace};

pub fn cmd_pre_enter(tag: &str, package_patterns: &[String], force: bool) -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;
  let workspace = load_workspace(&root)?;

  let (mut config, config_path) = OxrlsConfig::load(&root)?;
  if config_path.as_os_str().is_empty() {
    return Err(OxrlsError::Config(
      "No oxrls.json found. Run `oxrls init` first.".to_string(),
    ));
  }

  let resolved_packages =
    crate::commands::resolve_package_patterns(package_patterns, &workspace)?;

  let entry_idx = config.pre_mode.iter().position(|e| e.tag == tag);
  let idx = if let Some(i) = entry_idx {
    i
  } else {
    config.pre_mode.push(PreModeEntry {
      tag: tag.to_string(),
      packages: vec![],
    });
    config.pre_mode.len() - 1
  };

  if !force {
    for pkg_name in &resolved_packages {
      for (other_idx, entry) in config.pre_mode.iter().enumerate() {
        if other_idx == idx {
          continue;
        }
        if entry.packages.iter().any(|p| p == pkg_name) {
          return Err(OxrlsError::Config(format!(
            "Package \"{}\" is already in pre-mode \"{}\". Use --force to migrate.",
            pkg_name, entry.tag
          )));
        }
      }
    }
  }

  if force {
    for other_entry in &mut config.pre_mode {
      other_entry
        .packages
        .retain(|p| !resolved_packages.contains(p));
    }
    let release_dir = crate::get_release_dir(&root, &config, &config_path);
    let mut pre_state = PreState::load(&release_dir)?;
    for pkg_name in &resolved_packages {
      if pre_state.is_in_pre(pkg_name)
        && let Some(entry) = pre_state.pre_versions.get(pkg_name)
        && entry.tag != tag
      {
        pre_state.remove(pkg_name);
      }
    }
    pre_state.save(&release_dir)?;
  }

  let entry = &mut config.pre_mode[idx];
  for pkg_name in &resolved_packages {
    if !entry.packages.contains(pkg_name) {
      entry.packages.push(pkg_name.clone());
    }
  }

  OxrlsConfig::write_to(&config_path, &config, true)?;

  println!(
    "Entered pre-release mode \"{}\" for {} package(s):",
    tag,
    resolved_packages.len()
  );
  for pkg_name in &resolved_packages {
    println!("  {}", pkg_name);
  }

  Ok(())
}

pub fn cmd_pre_exit(package_patterns: &[String]) -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;
  let workspace = load_workspace(&root)?;

  let (mut config, config_path) = OxrlsConfig::load(&root)?;
  if config_path.as_os_str().is_empty() {
    return Err(OxrlsError::Config("No oxrls.json found.".to_string()));
  }

  let to_remove =
    crate::commands::resolve_package_patterns(package_patterns, &workspace)?;

  for entry in &mut config.pre_mode {
    entry.packages.retain(|p| !to_remove.contains(p));
  }
  config.pre_mode.retain(|e| !e.packages.is_empty());

  OxrlsConfig::write_to(&config_path, &config, true)?;

  let release_dir = crate::get_release_dir(&root, &config, &config_path);
  let mut pre_state = PreState::load(&release_dir)?;
  for pkg_name in &to_remove {
    pre_state.remove(pkg_name);
  }
  pre_state.save(&release_dir)?;

  println!(
    "Exited pre-release mode for {} package(s):",
    to_remove.len()
  );
  for pkg_name in &to_remove {
    println!("  {}", pkg_name);
  }

  Ok(())
}

pub fn cmd_pre_status() -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;

  let (config, config_path) = OxrlsConfig::load(&root)?;
  let release_dir = crate::get_release_dir(&root, &config, &config_path);

  if config.pre_mode.is_empty() {
    println!("No pre-release mode configured.");
    return Ok(());
  }

  let pre_state = PreState::load(&release_dir)?;

  println!("Pre-release mode:\n");
  for entry in &config.pre_mode {
    println!("  Tag \"{}\":", entry.tag);
    for pattern in &entry.packages {
      println!("    - {}", pattern);
    }
    println!();
  }

  if !pre_state.pre_versions.is_empty() {
    println!("Current pre-release counters:\n");
    for (pkg, pve) in &pre_state.pre_versions {
      println!("  {}  {} (counter: {})", pkg, pve.tag, pve.count);
    }
  }

  Ok(())
}

pub fn cmd_pre_interactive() -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;
  let workspace = load_workspace(&root)?;

  if workspace.packages.is_empty() {
    return Err(OxrlsError::Config(
      "No packages found in workspace.".to_string(),
    ));
  }

  let package_names: Vec<&String> = workspace.packages.keys().collect();
  let selected = MultiSelect::new(
    "Which packages should enter pre-release mode?",
    package_names,
  )
  .prompt()
  .map_err(|e| OxrlsError::Other(format!("Selection failed: {}", e)))?;

  if selected.is_empty() {
    return Err(OxrlsError::Other("No packages selected.".to_string()));
  }

  let tag = Text::new("Pre-release tag (e.g., beta, alpha, rc):")
    .with_placeholder("beta")
    .prompt()
    .map_err(|e| OxrlsError::Other(format!("Input failed: {}", e)))?;

  let tag = if tag.trim().is_empty() {
    "beta".to_string()
  } else {
    tag.trim().to_lowercase()
  };

  let package_patterns: Vec<String> = selected.iter().map(|s| (*s).clone()).collect();
  cmd_pre_enter(&tag, &package_patterns, false)
}
