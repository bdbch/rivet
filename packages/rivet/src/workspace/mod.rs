//! Workspace resolution and package.json handling.
//!
//! This module provides:
//! - [`Workspace`] / [`WorkspacePackage`] — types representing a workspace and its packages.
//! - [`find_workspace_root`], [`load_workspace`], [`get_workspace_globs`] — workspace discovery.
//! - [`PackageJson`] — model for `package.json` files with read/write/version helpers.
//! - [`compute_new_range`], [`get_range_prefix`], [`format_version_like`] — version-range bumping utilities.

pub use loader::*;
pub use package_json::*;

mod loader;
mod package_json;
