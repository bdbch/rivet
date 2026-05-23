#![deny(clippy::all)]

use napi_derive::napi;

pub mod bump;
pub mod changelog;
pub mod cli;
pub mod config;
pub mod error;
pub mod init_wizard;
pub mod package_json;
pub mod premode;
pub mod release;
pub mod release_file;
pub mod version_bump;
pub mod workspace;

use std::path::Path;

use clap::Parser;
use glob::Pattern;
use indexmap::IndexMap;
use inquire::{MultiSelect, Select, Text};

use crate::bump::{apply_release_plan, build_release_plan, find_release_files, print_plan};
use crate::cli::{Cli, Commands, PreAction};
use crate::config::{OxrlsConfig, PreModeEntry};
use crate::error::{OxrlsError, Result};
use crate::init_wizard::run_init_wizard;
use crate::package_json::PackageJson;
use crate::premode::PreState;
use crate::release::{ReleaseManifest, publish_manifest};
use crate::release_file::{BumpType, create_release_file, parse_release_file};
use crate::workspace::{Workspace, find_workspace_root, load_workspace};

/// Run the CLI with the given argument list.
/// Shared by both the standalone binary (main.rs) and the NAPI entry point.
pub fn run_with_args<I, S>(args: I) -> Result<()>
where
  I: IntoIterator<Item = S>,
  S: Into<std::ffi::OsString> + Clone,
{
  let cli = Cli::parse_from(args);

  match &cli.command {
    Commands::Init {
      force,
      release_dir,
      non_interactive,
    } => cmd_init(*force, release_dir.as_deref(), *non_interactive),
    Commands::New {
      packages,
      summary,
      details,
    } => cmd_new(packages, summary.as_deref(), details.as_deref()),
    Commands::Status => cmd_status(),
    Commands::Bump { dry_run, archive } => cmd_bump(*dry_run, *archive),
    Commands::Check => cmd_check(),
    Commands::Release { dry_run, tag } => cmd_release(*dry_run, tag.as_deref()),
    Commands::Pre { action } => match action {
      Some(PreAction::Enter {
        tag,
        packages,
        force,
      }) => cmd_pre_enter(tag, packages, *force),
      Some(PreAction::Exit { packages }) => cmd_pre_exit(packages),
      Some(PreAction::Status) => cmd_pre_status(),
      None => cmd_pre_interactive(),
    },
  }
}

#[napi]
pub fn run_cli(args: Vec<String>) -> napi::Result<()> {
  // Prepend the program name so clap parses correctly
  let full_args = std::iter::once("oxrls".to_string())
    .chain(args)
    .collect::<Vec<_>>();
  run_with_args(&full_args).map_err(|e| napi::Error::from_reason(format!("{:#}", e)))
}

#[napi]
pub fn plus_100(input: u32) -> u32 {
  input + 100
}

// ---------------------------------------------------------------------------
// Command implementations (extracted from main.rs)
// ---------------------------------------------------------------------------

fn get_release_dir(root: &Path, config: &OxrlsConfig, config_path: &Path) -> std::path::PathBuf {
  if !config_path.as_os_str().is_empty() {
    config.release_dir_abs(config_path)
  } else {
    root.join(&config.release_dir)
  }
}

fn cmd_init(force: bool, release_dir: Option<&str>, non_interactive: bool) -> Result<()> {
  let cwd = std::env::current_dir().map_err(OxrlsError::Io)?;

  let root = find_workspace_root(Path::new(".")).unwrap_or_else(|_| cwd.clone());
  let workspace = load_workspace(&root).unwrap_or_else(|_| Workspace {
    root: root.clone(),
    root_package_json: PackageJson {
      name: None,
      version: None,
      private: None,
      dependencies: None,
      dev_dependencies: None,
      peer_dependencies: None,
      optional_dependencies: None,
      extra: std::collections::BTreeMap::new(),
    },
    packages: IndexMap::new(),
  });
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

fn cmd_new(packages: &[String], summary: Option<&str>, details: Option<&str>) -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;
  let workspace = load_workspace(&root)?;

  let (config, config_path) = OxrlsConfig::load(&root)?;
  let release_dir = get_release_dir(&root, &config, &config_path);

  let releases: IndexMap<String, BumpType>;

  if packages.is_empty() {
    if workspace.packages.is_empty() {
      return Err(OxrlsError::ReleaseFile(
        "No workspace packages found. Run `oxrls init` first or add packages.".to_string(),
      ));
    }

    let package_names: Vec<&String> = workspace.packages.keys().collect();

    let selected = MultiSelect::new("Which packages changed?", package_names)
      .prompt()
      .map_err(|e| OxrlsError::Other(format!("Selection failed: {}", e)))?;

    if selected.is_empty() {
      return Err(OxrlsError::ReleaseFile("No packages selected.".to_string()));
    }

    let bump_options = vec!["patch", "minor", "major"];
    let bump = Select::new("Bump type for all selected packages:", bump_options)
      .prompt()
      .map_err(|e| OxrlsError::Other(format!("Selection failed: {}", e)))?;
    let bump_type: BumpType = bump.parse::<BumpType>()?;

    let mut releases_map: IndexMap<String, BumpType> = IndexMap::new();
    for pkg_name in selected {
      releases_map.insert(pkg_name.clone(), bump_type);
    }

    let summary_text = Text::new("Summary of the change:")
      .prompt()
      .map_err(|e| OxrlsError::Other(format!("Input failed: {}", e)))?;

    let details_text: Option<String> = Text::new("Optional details (enter to skip):")
      .prompt()
      .ok()
      .filter(|s: &String| !s.is_empty());

    releases = releases_map;
    let path = create_release_file(
      &release_dir,
      &releases,
      &summary_text,
      details_text.as_deref(),
    )?;
    println!("\nCreated release file: {}", path.display());
  } else {
    let mut releases_map: IndexMap<String, BumpType> = IndexMap::new();
    for pkg_arg in packages {
      let parts: Vec<&str> = pkg_arg.splitn(2, ':').collect();
      let pkg_name = parts[0];
      let bump_str = parts.get(1).ok_or_else(|| {
        OxrlsError::ReleaseFile(format!(
          "Invalid package format: \"{}\". Expected format: \"@scope/pkg:bumptype\"",
          pkg_arg
        ))
      })?;
      let bump_type = bump_str.parse::<BumpType>()?;
      releases_map.insert(pkg_name.to_string(), bump_type);
    }

    let summary_text = summary.unwrap_or("No summary provided.");

    releases = releases_map;
    let path = create_release_file(&release_dir, &releases, summary_text, details)?;
    println!("Created release file: {}", path.display());
  }

  Ok(())
}

fn cmd_status() -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;
  let workspace = load_workspace(&root)?;

  let (config, config_path) = OxrlsConfig::load(&root)?;
  let release_dir = get_release_dir(&root, &config, &config_path);

  let files = find_release_files(&release_dir)?;

  if files.is_empty() {
    println!(
      "No pending release files found in {}.\n",
      release_dir.display()
    );
    return Ok(());
  }

  println!("Pending release files:\n");

  for file_path in &files {
    match parse_release_file(file_path) {
      Ok(rf) => {
        let fname = file_path
          .file_name()
          .map(|n| n.to_string_lossy())
          .unwrap_or_default();
        println!("  {}", fname);
        for (pkg, bump) in &rf.releases {
          println!("    {}  {}", pkg, bump);
        }
      }
      Err(e) => {
        eprintln!("  ERROR parsing {}: {}", file_path.display(), e);
      }
    }
  }

  println!();

  match build_release_plan(&workspace, &config, &release_dir, true) {
    Ok(plan) => {
      let _max_name = plan
        .bumps
        .values()
        .map(|b| b.package_name.len())
        .max()
        .unwrap_or(20);
      println!("Calculated bumps:");
      for (_name, bump) in &plan.bumps {
        println!(
          "  {}  {} → {}  ({})",
          bump.package_name,
          bump.old_version,
          bump.new_version,
          bump.bump_type_str()
        );
      }
      if !plan.internal_dep_updates.is_empty() {
        println!("\nInternal dependency updates:\n");
        for update in &plan.internal_dep_updates {
          println!(
            "  {} will update {} ({})",
            update.dependent_package_name, update.dep_name, update.dep_type
          );
        }
      }
    }
    Err(e) => {
      eprintln!("\nCould not calculate bumps: {}", e);
      std::process::exit(1);
    }
  }

  Ok(())
}

fn cmd_bump(dry_run: bool, archive: bool) -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;
  let workspace = load_workspace(&root)?;

  let (config, config_path) = OxrlsConfig::load(&root)?;
  let release_dir = get_release_dir(&root, &config, &config_path);

  let plan = build_release_plan(&workspace, &config, &release_dir, dry_run)?;

  if dry_run {
    println!("[DRY RUN] Would apply the following release plan:\n");
    print_plan(&plan);
    return Ok(());
  }

  println!("Bumped packages:");
  for (_name, bump) in &plan.bumps {
    println!(
      "  {}  {} → {}  ({})",
      bump.package_name,
      bump.old_version,
      bump.new_version,
      bump.bump_type_str()
    );
  }
  let plan_clone = plan.clone();
  apply_release_plan(&workspace, &plan, &config, &release_dir, false, archive)?;

  if !plan_clone.internal_dep_updates.is_empty() {
    println!("\nUpdated internal dependencies:");
    for update in &plan_clone.internal_dep_updates {
      println!(
        "  {} {}",
        update.dependent_package_path.display(),
        update.dep_name
      );
    }
  }

  println!("\nConsumed release files:");
  let mut seen: std::collections::HashSet<std::path::PathBuf> = std::collections::HashSet::new();
  for (_name, bump) in &plan_clone.bumps {
    for rf_path in &bump.release_files {
      if seen.insert(rf_path.clone()) {
        println!("  {}", rf_path.display());
      }
    }
  }

  println!("\nDone!");
  Ok(())
}

fn cmd_check() -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;
  let (config, config_path) = OxrlsConfig::load(&root)?;
  let release_dir = get_release_dir(&root, &config, &config_path);

  let release_files = find_release_files(&release_dir).unwrap_or_default();
  let has_release_plan = ReleaseManifest::path(&release_dir).exists();

  if !release_files.is_empty() {
    println!("Release files exist, skip release");
    std::process::exit(0);
  }

  if has_release_plan {
    println!("Release plan exists and files are clean, can release");
    std::process::exit(1);
  }

  println!("Nothing to release");
  std::process::exit(0);
}

fn cmd_release(dry_run: bool, tag_override: Option<&str>) -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;
  let workspace = load_workspace(&root)?;

  let (config, config_path) = OxrlsConfig::load(&root)?;
  let release_dir = get_release_dir(&root, &config, &config_path);

  let manifest = ReleaseManifest::load(&release_dir)?;

  if manifest.packages.is_empty() {
    println!("No packages to release.");
    return Ok(());
  }

  println!("Releasing {} package(s):\n", manifest.packages.len());
  publish_manifest(&manifest, &workspace, &config, dry_run, tag_override)
}

fn cmd_pre_enter(tag: &str, package_patterns: &[String], force: bool) -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;
  let workspace = load_workspace(&root)?;

  let (mut config, config_path) = OxrlsConfig::load(&root)?;
  if config_path.as_os_str().is_empty() {
    return Err(OxrlsError::Config(
      "No oxrls.json found. Run `oxrls init` first.".to_string(),
    ));
  }

  let mut resolved_packages: Vec<String> = Vec::new();
  for pattern in package_patterns {
    if pattern.contains('*') || pattern.contains('?') {
      let pat = Pattern::new(pattern)
        .map_err(|e| OxrlsError::Config(format!("Invalid glob pattern: {}", e)))?;
      for name in workspace.packages.keys() {
        if pat.matches(name) && !resolved_packages.contains(name) {
          resolved_packages.push(name.clone());
        }
      }
    } else if workspace.packages.contains_key(pattern) {
      if !resolved_packages.contains(pattern) {
        resolved_packages.push(pattern.clone());
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
        if !resolved_packages.contains(&name) {
          resolved_packages.push(name);
        }
      }
    }
  }

  if resolved_packages.is_empty() {
    return Err(OxrlsError::Config(
      "No packages matched the given patterns.".to_string(),
    ));
  }

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
    let mut pre_state = PreState::load(
      &config_path
        .parent()
        .unwrap_or(&root)
        .join(&config.release_dir),
    )?;
    for pkg_name in &resolved_packages {
      if pre_state.is_in_pre(pkg_name)
        && let Some(entry) = pre_state.pre_versions.get(pkg_name)
        && entry.tag != tag
      {
        pre_state.remove(pkg_name);
      }
    }
    pre_state.save(
      &config_path
        .parent()
        .unwrap_or(&root)
        .join(&config.release_dir),
    )?;
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

fn cmd_pre_exit(package_patterns: &[String]) -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;
  let workspace = load_workspace(&root)?;

  let (mut config, config_path) = OxrlsConfig::load(&root)?;
  if config_path.as_os_str().is_empty() {
    return Err(OxrlsError::Config("No oxrls.json found.".to_string()));
  }

  let mut to_remove: Vec<String> = Vec::new();
  for pattern in package_patterns {
    if pattern.contains('*') || pattern.contains('?') {
      let pat = Pattern::new(pattern)
        .map_err(|e| OxrlsError::Config(format!("Invalid glob pattern: {}", e)))?;
      for name in workspace.packages.keys() {
        if pat.matches(name) && !to_remove.contains(name) {
          to_remove.push(name.clone());
        }
      }
    } else if workspace.packages.contains_key(pattern) {
      if !to_remove.contains(pattern) {
        to_remove.push(pattern.clone());
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
        if !to_remove.contains(&name) {
          to_remove.push(name);
        }
      }
    }
  }

  for entry in &mut config.pre_mode {
    entry.packages.retain(|p| !to_remove.contains(p));
  }
  config.pre_mode.retain(|e| !e.packages.is_empty());

  OxrlsConfig::write_to(&config_path, &config, true)?;

  let mut pre_state = PreState::load(
    &config_path
      .parent()
      .unwrap_or(&root)
      .join(&config.release_dir),
  )?;
  for pkg_name in &to_remove {
    pre_state.remove(pkg_name);
  }
  pre_state.save(
    &config_path
      .parent()
      .unwrap_or(&root)
      .join(&config.release_dir),
  )?;

  println!(
    "Exited pre-release mode for {} package(s):",
    to_remove.len()
  );
  for pkg_name in &to_remove {
    println!("  {}", pkg_name);
  }

  Ok(())
}

fn cmd_pre_status() -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;

  let (config, config_path) = OxrlsConfig::load(&root)?;
  let release_dir = get_release_dir(&root, &config, &config_path);

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

fn cmd_pre_interactive() -> Result<()> {
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
