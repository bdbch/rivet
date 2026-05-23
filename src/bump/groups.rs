use crate::bump::plan::PlannedBump;
use crate::config::OxrlsConfig;
use crate::error::{OxrlsError, Result};
use crate::release_file::BumpType;
use crate::version_bump::bump_version;
use crate::workspace::{Workspace, WorkspacePackage};
use glob::Pattern;
use indexmap::IndexMap;
use std::collections::HashSet;
use std::path::PathBuf;

/// Resolve a group of package name patterns (with optional `!` negation)
/// against the workspace package names.
///
/// Supports glob patterns via `glob::Pattern`:
/// - `"@scope/*"` matches all packages under `@scope/`
/// - `"!@scope/special"` excludes `@scope/special` from the resolved set
///
/// The resolution order is: all inclusions are applied first, then exclusions.
pub(crate) fn resolve_group_patterns(
  patterns: &[String],
  packages: &IndexMap<String, WorkspacePackage>,
) -> Result<Vec<String>> {
  if patterns.is_empty() {
    return Ok(vec![]);
  }

  // If no patterns use glob or negation, skip resolution and return as-is
  let needs_resolution = patterns
    .iter()
    .any(|p| p.contains('*') || p.contains('?') || p.contains('[') || p.starts_with('!'));

  if !needs_resolution {
    return Ok(patterns.to_vec());
  }

  let mut result: Vec<String> = Vec::new();

  // Phase 1: process inclusion patterns (no `!` prefix)
  let inclusion_patterns: Vec<&str> = patterns
    .iter()
    .filter(|p| !p.starts_with('!'))
    .map(|p| p.as_str())
    .collect();

  if inclusion_patterns.is_empty() {
    // No explicit inclusions means all packages
    for name in packages.keys() {
      result.push(name.clone());
    }
  } else {
    for pattern_str in &inclusion_patterns {
      let pat = Pattern::new(pattern_str).map_err(|e| {
        OxrlsError::Config(format!("Invalid glob pattern \"{}\": {}", pattern_str, e))
      })?;
      for name in packages.keys() {
        if pat.matches(name) {
          result.push(name.clone());
        }
      }
    }
  }

  // Phase 2: process exclusion patterns (prefixed with `!`)
  let exclusion_patterns: Vec<&str> = patterns
    .iter()
    .filter(|p| p.starts_with('!'))
    .map(|p| &p[1..])
    .collect();

  if !exclusion_patterns.is_empty() {
    result.retain(|name| {
      !exclusion_patterns.iter().any(|pat_str| {
        Pattern::new(pat_str)
          .map(|pat| pat.matches(name))
          .unwrap_or_else(|e| {
            eprintln!("Warning: invalid glob pattern \"!{}\": {}", pat_str, e);
            false
          })
      })
    });
  }

  result.sort();
  result.dedup();
  Ok(result)
}

/// Apply fixed group constraints: all packages in a fixed group share the same version.
/// If any member of a fixed group is bumped, every member gets bumped to the same new version
/// (computed from the highest bump type × the highest old version in the group).
///
/// Supports glob patterns and `!` negation in group definitions:
/// ```json
/// { "fixed": [["@scope/*", "!@scope/special"]] }
/// ```
pub(crate) fn apply_fixed_groups(
  bumps: &mut IndexMap<String, PlannedBump>,
  workspace: &Workspace,
  config: &OxrlsConfig,
) -> Result<()> {
  for group_patterns in &config.fixed {
    if group_patterns.is_empty() {
      continue;
    }

    let group = resolve_group_patterns(group_patterns, &workspace.packages)?;

    // Collect current state of all group members
    let mut group_bumps: Vec<(String, semver::Version, BumpType)> = Vec::new();
    let mut any_bumped = false;

    for pkg_name in &group {
      if let Some(bump) = bumps.get(pkg_name) {
        any_bumped = true;
        group_bumps.push((pkg_name.clone(), bump.old_version.clone(), bump.bump_type));
      } else if let Some(pkg) = workspace.packages.get(pkg_name)
        && let Ok(ver) = pkg.package_json.semver_version()
      {
        group_bumps.push((pkg_name.clone(), ver, BumpType::Patch));
      }
    }

    // Only apply fixed constraint if at least one member was bumped
    if !any_bumped {
      continue;
    }

    // Find the highest old version and max bump type in the group
    let mut max_bump = BumpType::Patch;
    let mut highest_old_version = semver::Version::new(0, 0, 0);

    for (_, old_ver, bump_type) in &group_bumps {
      if old_ver > &highest_old_version {
        highest_old_version = old_ver.clone();
      }
      if bump_type.priority() > max_bump.priority() {
        max_bump = *bump_type;
      }
    }

    // Compute the shared new version
    let shared_new_version = bump_version(&highest_old_version, max_bump);

    // Collect summaries from the packages that were directly bumped in this group
    let direct_summaries: Vec<String> = group_bumps
      .iter()
      .filter(|(name, _, _)| bumps.contains_key(name.as_str()))
      .flat_map(|(name, _, _)| {
        bumps
          .get(name)
          .map(|b| b.summaries.clone())
          .unwrap_or_default()
      })
      .collect();
    let direct_release_files: Vec<PathBuf> = group_bumps
      .iter()
      .filter(|(name, _, _)| bumps.contains_key(name.as_str()))
      .flat_map(|(name, _, _)| {
        bumps
          .get(name)
          .map(|b| b.release_files.clone())
          .unwrap_or_default()
      })
      .collect();

    // Snapshot which packages were originally in the bump plan (before we mutate `bumps`)
    let originally_bumped: HashSet<&str> = group_bumps
      .iter()
      .filter(|(name, _, _)| bumps.contains_key(name.as_str()))
      .map(|(name, _, _)| name.as_str())
      .collect();

    // Apply to all group members
    for (pkg_name, old_ver, _) in &group_bumps {
      let existing_summaries = bumps
        .get(pkg_name)
        .map(|b| b.summaries.clone())
        .unwrap_or_default();
      let existing_release_files = bumps
        .get(pkg_name)
        .map(|b| b.release_files.clone())
        .unwrap_or_default();

      // If this package wasn't directly bumped (no release file entries),
      // derive the summary from what was bumped in the group
      let summaries = if existing_summaries.is_empty() {
        let deps: Vec<&str> = originally_bumped
          .iter()
          .filter(|name| **name != *pkg_name)
          .copied()
          .collect();
        if !deps.is_empty() {
          vec![format!("Updated with {}.", deps.join(", "))]
        } else if !direct_summaries.is_empty() {
          direct_summaries.clone()
        } else {
          vec!["Updated to match fixed group version.".to_string()]
        }
      } else {
        existing_summaries
      };

      let release_files = if existing_release_files.is_empty() {
        direct_release_files.clone()
      } else {
        existing_release_files
      };

      bumps.insert(
        pkg_name.clone(),
        PlannedBump {
          package_name: pkg_name.clone(),
          old_version: old_ver.clone(),
          new_version: shared_new_version.clone(),
          bump_type: max_bump,
          summaries,
          release_files,
        },
      );
    }
  }

  Ok(())
}

/// Apply linked group constraints: all packages in a linked group share the same bump type.
/// If any member of a linked group receives a bump, every other member in the group
/// that is also being bumped gets the highest bump type found in the group.
///
/// Supports glob patterns and `!` negation in group definitions:
/// ```json
/// { "linked": [["@scope/*", "!@scope/special"]] }
/// ```
pub(crate) fn apply_linked_groups(
  bumps: &mut IndexMap<String, PlannedBump>,
  workspace: &Workspace,
  config: &OxrlsConfig,
) -> Result<()> {
  for group_patterns in &config.linked {
    if group_patterns.is_empty() {
      continue;
    }

    let group = resolve_group_patterns(group_patterns, &workspace.packages)?;

    // Find the max bump type among group members that are in the plan
    let mut max_bump: Option<BumpType> = None;
    for pkg_name in &group {
      if let Some(bump) = bumps.get(pkg_name) {
        max_bump = Some(BumpType::max(max_bump, bump.bump_type));
      }
    }

    let max_bump = match max_bump {
      Some(b) => b,
      None => continue,
    };

    // Apply the max bump type to all group members that are in the plan
    for pkg_name in &group {
      if let Some(bump) = bumps.get_mut(pkg_name)
        && bump.bump_type != max_bump
      {
        bump.bump_type = max_bump;
        bump.new_version = bump_version(&bump.old_version, max_bump);
      }
    }
  }

  Ok(())
}
