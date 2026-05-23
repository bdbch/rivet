//! `oxrls check` — CI-friendly status check for release readiness.
//!
//! Returns a `CheckStatus` indicating whether there are pending releases,
//! a ready release plan, or nothing to do. This is used by CI pipelines
//! to decide whether to trigger a publish step.

use std::path::Path;

use crate::bump::find_release_files;
use crate::config::OxrlsConfig;
use crate::error::Result;
use crate::release::ReleaseManifest;
use crate::workspace::find_workspace_root;

pub fn cmd_check() -> Result<crate::CheckStatus> {
  let root = find_workspace_root(Path::new("."))?;
  let (config, config_path) = OxrlsConfig::load(&root)?;
  let release_dir = crate::get_release_dir(&root, &config, &config_path);

  let release_files = find_release_files(&release_dir).unwrap_or_default();
  let has_release_plan = ReleaseManifest::path(&release_dir).exists();

  if !release_files.is_empty() {
    println!("Release files exist, skip release");
    return Ok(crate::CheckStatus::PendingReleases);
  }

  if has_release_plan {
    println!("Release plan exists and files are clean, can release");
    return Ok(crate::CheckStatus::ReadyToRelease);
  }

  println!("Nothing to release");
  Ok(crate::CheckStatus::NothingToRelease)
}
