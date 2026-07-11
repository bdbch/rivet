//! `rivet new` — create a new release file for one or more packages.
//!
//! In interactive mode (no package arguments) it prompts for packages,
//! bump type, summary, and details. In CLI mode it accepts
//! `@scope/pkg:bumptype` pairs plus `--summary` / `--details`.

use std::path::Path;

use indexmap::IndexMap;
use inquire::{MultiSelect, Select, Text};

use crate::config::RivetConfig;
use crate::error::{Result, RivetError};
use crate::release_file::{BumpType, create_release_file};
use crate::workspace::{find_workspace_root, load_workspace};

pub fn cmd_new(packages: &[String], summary: Option<&str>, details: Option<&str>) -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;
  let workspace = load_workspace(&root)?;

  let (config, config_path) = RivetConfig::load(&root)?;
  let release_dir = crate::get_release_dir(&root, &config, &config_path);

  let releases: IndexMap<String, BumpType>;

  if packages.is_empty() {
    if workspace.packages.is_empty() {
      return Err(RivetError::ReleaseFile(
        "No workspace packages found. Run `rivet init` first or add packages.".to_string(),
      ));
    }

    let package_names: Vec<&String> = workspace.packages.keys().collect();

    let selected = MultiSelect::new("Which packages changed?", package_names)
      .prompt()
      .map_err(|e| RivetError::Other(format!("Selection failed: {}", e)))?;

    if selected.is_empty() {
      return Err(RivetError::ReleaseFile("No packages selected.".to_string()));
    }

    let bump_options = vec!["patch", "minor", "major"];
    let bump = Select::new("Bump type for all selected packages:", bump_options)
      .prompt()
      .map_err(|e| RivetError::Other(format!("Selection failed: {}", e)))?;
    let bump_type: BumpType = bump.parse::<BumpType>()?;

    let mut releases_map: IndexMap<String, BumpType> = IndexMap::new();
    for pkg_name in selected {
      releases_map.insert(pkg_name.clone(), bump_type);
    }

    let summary_text = Text::new("Summary of the change:")
      .prompt()
      .map_err(|e| RivetError::Other(format!("Input failed: {}", e)))?;

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
        RivetError::ReleaseFile(format!(
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
