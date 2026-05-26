use crate::bump::deps::compute_internal_dep_updates;
use crate::bump::discovery::find_release_files;
use crate::bump::groups::{apply_fixed_groups, apply_linked_groups};
use crate::config::OxrlsConfig;
use crate::error::{OxrlsError, Result};
use crate::prerelease::{PreState, apply_pre_release, resolve_pre_release};
use crate::release_file::{BumpType, ReleaseFile, parse_release_file};
use crate::version_bump::bump_version;
use crate::workspace::Workspace;
use indexmap::IndexMap;
use std::path::{Path, PathBuf};

/// A planned version bump for a single package.
#[derive(Debug, Clone)]
pub struct PlannedBump {
  pub package_name: String,
  pub old_version: semver::Version,
  pub new_version: semver::Version,
  pub bump_type: BumpType,
  pub summaries: Vec<String>,
  /// The release files that caused this bump.
  pub release_files: Vec<PathBuf>,
}

/// A complete release plan built from pending release files.
#[derive(Debug, Clone)]
pub struct ReleasePlan {
  pub bumps: IndexMap<String, PlannedBump>,
  pub internal_dep_updates: Vec<InternalDepUpdateInfo>,
  /// The pre-release state after counter increments.
  /// We carry this through to `apply_release_plan` so the counters
  /// only get persisted after a successful apply (atomicity).
  pub pre_state: PreState,
}

/// Information about an internal dependency range update.
#[derive(Debug, Clone)]
pub struct InternalDepUpdateInfo {
  pub dependent_package_path: PathBuf,
  pub dependent_package_name: String,
  pub dep_name: String,
  pub dep_type: String,
  pub old_range: String,
  pub new_range: String,
}

/// Build a release plan by reading all release files and computing bumps.
pub fn build_release_plan(
  workspace: &Workspace,
  config: &OxrlsConfig,
  release_dir: &Path,
) -> Result<ReleasePlan> {
  // Find all release files
  let release_files = find_release_files(release_dir)?;

  if release_files.is_empty() {
    return Err(OxrlsError::Bump(
      "No pending release files found.".to_string(),
    ));
  }

  // Parse all release files
  let parsed: Vec<ReleaseFile> = release_files
    .iter()
    .map(|path| parse_release_file(path))
    .collect::<Result<Vec<_>>>()?;

  // Validate all package references
  for rf in &parsed {
    for pkg_name in rf.releases.keys() {
      if !workspace.packages.contains_key(pkg_name) {
        return Err(OxrlsError::Bump(format!(
          "Release file {} references \"{}\", but no workspace package with that name exists.",
          rf.path.display(),
          pkg_name
        )));
      }
    }
  }

  // Merge bump types per package (highest priority wins)
  let mut merged_bumps: IndexMap<String, (BumpType, Vec<&ReleaseFile>)> = IndexMap::new();
  for rf in &parsed {
    for (pkg_name, bump_type) in &rf.releases {
      let entry = merged_bumps
        .entry(pkg_name.clone())
        .or_insert((*bump_type, vec![]));
      entry.0 = BumpType::max(Some(entry.0), *bump_type);
      entry.1.push(rf);
    }
  }

  // Compute new versions
  let mut bumps: IndexMap<String, PlannedBump> = IndexMap::new();
  let mut pre_state = PreState::load(release_dir)?;

  for (pkg_name, (bump_type, refs)) in &merged_bumps {
    let pkg = workspace
      .packages
      .get(pkg_name)
      .ok_or_else(|| OxrlsError::Bump(format!("Package '{}' not found in workspace", pkg_name)))?;

    let old_version = pkg.package_json.semver_version()?;

    // Apply pre-release tag if the package is in pre-mode
    // On the FIRST pre-release bump (count == 1), apply the bump type
    // to the base version before adding the pre-release tag.
    // Subsequent bumps only increment the pre-release counter.
    let new_version =
      if let Some((tag, count)) = resolve_pre_release(pkg_name, config, &mut pre_state) {
        if count == 1 {
          // First pre-release: bump the base version, then add pre-release tag
          let bumped = bump_version(&old_version, *bump_type);
          let base = semver::Version::new(bumped.major, bumped.minor, bumped.patch);
          apply_pre_release(&base, &tag, count)?
        } else {
          // Subsequent pre-release: keep the base version, just increment counter
          let base = semver::Version::new(old_version.major, old_version.minor, old_version.patch);
          apply_pre_release(&base, &tag, count)?
        }
      } else {
        // Normal bump — not in pre-release mode
        bump_version(&old_version, *bump_type)
      };

    // Collect summaries from the release files that reference this package
    let summaries: Vec<String> = refs.iter().map(|rf| rf.summary.clone()).collect();

    let release_files: Vec<PathBuf> = refs.iter().map(|rf| rf.path.clone()).collect();

    bumps.insert(
      pkg_name.clone(),
      PlannedBump {
        package_name: pkg_name.clone(),
        old_version,
        new_version,
        bump_type: *bump_type,
        summaries,
        release_files,
      },
    );
  }

  // NOTE: We do NOT save pre_state here, even if !dry_run.
  // The pre_state is carried forward in ReleasePlan and saved in
  // `apply_release_plan` after all writes succeed. This ensures
  // the counter is only persisted when the bump actually completes
  // (atomicity — if applying the plan fails, the counter stays
  // unchanged, so a retry produces the same pre-release version).

  // Apply fixed group constraints — all packages in a fixed group share the same version
  apply_fixed_groups(&mut bumps, workspace, config)?;

  // Apply linked group constraints — all packages in a linked group share the same bump type
  apply_linked_groups(&mut bumps, workspace, config)?;

  // Compute internal dependency updates
  let internal_updates = compute_internal_dep_updates(workspace, &bumps, config)?;

  Ok(ReleasePlan {
    bumps,
    internal_dep_updates: internal_updates,
    pre_state,
  })
}
