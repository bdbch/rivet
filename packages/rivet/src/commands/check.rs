//! `rivet check` — CI-friendly status check for release readiness.
//!
//! Returns a `CheckStatus` indicating whether there are pending releases,
//! a ready release plan, or nothing to do. This is used by CI pipelines
//! to decide whether to trigger a publish step.

use std::path::Path;

use crate::bump::find_release_files;
use crate::config::RivetConfig;
use crate::error::Result;
use crate::release::ReleaseManifest;
use crate::workspace::find_workspace_root;

pub fn cmd_check(json: bool) -> Result<crate::CheckStatus> {
  let root = find_workspace_root(Path::new("."))?;
  let (config, config_path) = RivetConfig::load(&root)?;
  let release_dir = crate::get_release_dir(&root, &config, &config_path);

  let release_files = find_release_files(&release_dir).unwrap_or_default();
  let has_release_plan = ReleaseManifest::path(&release_dir).exists();

  let status = if !release_files.is_empty() {
    crate::CheckStatus::PendingReleases
  } else if has_release_plan {
    crate::CheckStatus::ReadyToRelease
  } else {
    crate::CheckStatus::NothingToRelease
  };

  if json {
    let status_name = match status {
      crate::CheckStatus::PendingReleases => "pending_releases",
      crate::CheckStatus::ReadyToRelease => "ready_to_release",
      crate::CheckStatus::NothingToRelease => "nothing_to_release",
    };
    println!(r#"{{"status":"{}"}}"#, status_name);
  } else {
    match status {
      crate::CheckStatus::PendingReleases => println!("Release files exist, skip release"),
      crate::CheckStatus::ReadyToRelease => {
        println!("Release plan exists and files are clean, can release")
      }
      crate::CheckStatus::NothingToRelease => println!("Nothing to release"),
    }
  }

  Ok(status)
}
