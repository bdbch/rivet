//! `oxrls release` — publish packages from a release manifest.
//!
//! Reads the release manifest created by a previous `oxrls bump` and
//! publishes each package. Supports `--dry-run` and `--tag` override.

use std::path::Path;

use crate::config::OxrlsConfig;
use crate::error::Result;
use crate::release::{ReleaseManifest, publish_manifest};
use crate::workspace::{find_workspace_root, load_workspace};

pub fn cmd_release(dry_run: bool, tag_override: Option<&str>) -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;
  let workspace = load_workspace(&root)?;

  let (config, config_path) = OxrlsConfig::load(&root)?;
  let release_dir = crate::get_release_dir(&root, &config, &config_path);

  let manifest = ReleaseManifest::load(&release_dir)?;

  if manifest.packages.is_empty() {
    println!("No packages to release.");
    return Ok(());
  }

  println!("Releasing {} package(s):\n", manifest.packages.len());
  publish_manifest(
    &manifest,
    &workspace,
    &config,
    &release_dir,
    dry_run,
    tag_override,
  )
}
