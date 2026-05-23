#![deny(clippy::all)]

use napi_derive::napi;

pub mod bump;
pub mod changelog;
pub mod cli;
pub mod commands;
pub mod config;
pub mod error;
pub mod init_wizard;
pub mod prerelease;
pub mod release;
pub mod release_file;
pub mod version_bump;
pub mod workspace;

use std::path::Path;

use clap::Parser;

use crate::cli::{Cli, Commands, PreAction};
use crate::config::OxrlsConfig;
use crate::error::{OxrlsError, Result};

// Bring all command functions into scope
use crate::commands::*;

/// Result of the `check` command — determines what CI should do.
/// These replace exit-code-based signaling that was previously done
/// with `std::process::exit()` in library code (which is dangerous
/// when called from NAPI — it would kill the entire Node.js process).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
  /// Release files exist → don't release yet, there are pending changes.
  PendingReleases,
  /// Release plan exists and files are clean → ready to publish.
  ReadyToRelease,
  /// No pending releases → nothing to do.
  NothingToRelease,
}

/// The result of running a command — used to carry extra info like
/// exit codes from `check` up to the caller (main.rs) without using
/// `process::exit()` inside library functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmdResult {
  /// Command completed normally.
  Ok,
  /// The `check` command ran and produced a status.
  CheckStatus(CheckStatus),
}

/// Run the CLI with the given argument list.
/// Shared by both the standalone binary (main.rs) and the NAPI entry point.
/// Returns a `CmdResult` so that commands like `check` can signal
/// exit-code-style results (e.g., "ready to publish") without calling
/// `std::process::exit()` inside library code.
pub fn run_with_args<I, S>(args: I) -> Result<CmdResult>
where
  I: IntoIterator<Item = S>,
  S: Into<std::ffi::OsString> + Clone,
{
  let cli = Cli::try_parse_from(args).map_err(|e| {
    // Use eprint so help/version text still reaches stderr
    // (clap::Error::print() would call process::exit, so we manually format)
    OxrlsError::Cli(e.to_string())
  })?;

  match &cli.command {
    Commands::Init {
      force,
      release_dir,
      non_interactive,
    } => {
      cmd_init(*force, release_dir.as_deref(), *non_interactive)?;
      Ok(CmdResult::Ok)
    }
    Commands::New {
      packages,
      summary,
      details,
    } => {
      cmd_new(packages, summary.as_deref(), details.as_deref())?;
      Ok(CmdResult::Ok)
    }
    Commands::Status => {
      cmd_status()?;
      Ok(CmdResult::Ok)
    }
    Commands::Bump { dry_run, archive } => {
      cmd_bump(*dry_run, *archive)?;
      Ok(CmdResult::Ok)
    }
    Commands::Check => Ok(CmdResult::CheckStatus(cmd_check()?)),
    Commands::Release { dry_run, tag } => {
      cmd_release(*dry_run, tag.as_deref())?;
      Ok(CmdResult::Ok)
    }
    Commands::Pre { action } => {
      match action {
        Some(PreAction::Enter {
          tag,
          packages,
          force,
        }) => cmd_pre_enter(tag, packages, *force)?,
        Some(PreAction::Exit { packages }) => cmd_pre_exit(packages)?,
        Some(PreAction::Status) => cmd_pre_status()?,
        None => cmd_pre_interactive()?,
      }
      Ok(CmdResult::Ok)
    }
  }
}

#[napi]
pub fn run_cli(args: Vec<String>) -> napi::Result<()> {
  // Prepend the program name so clap parses correctly
  let full_args = std::iter::once("oxrls".to_string())
    .chain(args)
    .collect::<Vec<_>>();
  // For the NAPI entry point, we ignore CmdResult and just propagate errors.
  // Exit-code-style results from `check` are irrelevant when called from Node.
  run_with_args(&full_args).map_err(|e| napi::Error::from_reason(format!("{:#}", e)))?;
  Ok(())
}

/// Resolve the absolute path to the release directory.
/// Uses the config's `release_dir` field, resolving relative to the
/// config file location when a config path is available.
pub(crate) fn get_release_dir(
  root: &Path,
  config: &OxrlsConfig,
  config_path: &Path,
) -> std::path::PathBuf {
  if !config_path.as_os_str().is_empty() {
    config.release_dir_abs(config_path)
  } else {
    root.join(&config.release_dir)
  }
}
