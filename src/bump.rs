use crate::changelog::{
  generate_changelog_section, generate_global_changelog_section, update_changelog,
  update_global_changelog, ChangelogEntry,
};
use crate::config::{InternalDepUpdate, OxrlsConfig};
use crate::error::{OxrlsError, Result};
use crate::package_json::PackageJson;
use crate::premode::{apply_pre_release, resolve_pre_release, PreState};
use crate::release::ReleaseManifest;
use crate::release_file::{consume_release_file, parse_release_file, BumpType, ReleaseFile};
use crate::version_bump::bump_version;
use crate::workspace::{Workspace, WorkspacePackage};
use glob::Pattern;
use indexmap::IndexMap;
use std::collections::HashSet;
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
  dry_run: bool,
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
    // When already in pre-release, keep the base version and just increment the counter
    let new_version = if let Some((tag, count)) = resolve_pre_release(pkg_name, config, &mut pre_state, workspace) {
      // Strip pre-release suffix, use the base version, then re-apply with new counter
      let base = semver::Version::new(old_version.major, old_version.minor, old_version.patch);
      apply_pre_release(&base, &tag, count)
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

  // Save pre-state after all bumps (counters are already incremented)
  if !dry_run {
    pre_state.save(release_dir)?;
  }

  // Apply fixed group constraints — all packages in a fixed group share the same version
  apply_fixed_groups(&mut bumps, workspace, config)?;

  // Apply linked group constraints — all packages in a linked group share the same bump type
  apply_linked_groups(&mut bumps, workspace, config)?;

  // Compute internal dependency updates
  let internal_updates = compute_internal_dep_updates(workspace, &bumps, config)?;

  Ok(ReleasePlan {
    bumps,
    internal_dep_updates: internal_updates,
  })
}

/// Resolve a group of package name patterns (with optional `!` negation)
/// against the workspace package names.
///
/// Supports glob patterns via `glob::Pattern`:
/// - `"@scope/*"` matches all packages under `@scope/`
/// - `"!@scope/special"` excludes `@scope/special` from the resolved set
///
/// The resolution order is: all inclusions are applied first, then exclusions.
fn resolve_group_patterns(
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
          .unwrap_or(false)
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
fn apply_fixed_groups(
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
        group_bumps.push((
          pkg_name.clone(),
          bump.old_version.clone(),
          bump.bump_type,
        ));
      } else if let Some(pkg) = workspace.packages.get(pkg_name)
        && let Ok(ver) = pkg.package_json.semver_version() {
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
    let originally_bumped: std::collections::HashSet<&str> = group_bumps
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
fn apply_linked_groups(
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
        && bump.bump_type != max_bump {
          bump.bump_type = max_bump;
          bump.new_version = bump_version(&bump.old_version, max_bump);
        }
    }
  }

  Ok(())
}

/// Compute which internal dependency ranges need updating.
fn compute_internal_dep_updates(
  workspace: &Workspace,
  bumps: &IndexMap<String, PlannedBump>,
  config: &OxrlsConfig,
) -> Result<Vec<InternalDepUpdateInfo>> {
  let mut updates = Vec::new();

  // Check all workspace packages for dependencies on bumped packages
  for (dep_name, pkg) in &workspace.packages {
    // If this package is itself bumped, no need to update self-dependency
    // Check all dependency fields
    let dep_fields: Vec<(&str, &Option<IndexMap<String, String>>)> = vec![
      ("dependencies", &pkg.package_json.dependencies),
      ("devDependencies", &pkg.package_json.dev_dependencies),
      ("peerDependencies", &pkg.package_json.peer_dependencies),
      (
        "optionalDependencies",
        &pkg.package_json.optional_dependencies,
      ),
    ];

    for (field_name, field) in &dep_fields {
      let deps = match field {
        Some(d) => d,
        None => continue,
      };

      for (dep_name_in_range, _range) in deps {
        // Check if this dependency is being bumped
        if let Some(bump) = bumps.get(dep_name_in_range) {
          // Check if we should update based on config
          if should_update_dependency(&config.update_internal_dependencies, &bump.bump_type) {
            // We'll compute the new range when applying
            updates.push(InternalDepUpdateInfo {
              dependent_package_path: pkg.dir.join("package.json"),
              dependent_package_name: dep_name.clone(),
              dep_name: dep_name_in_range.clone(),
              dep_type: field_name.to_string(),
              old_range: _range.clone(),
              new_range: String::new(), // filled in during apply
            });
          }
        }
      }
    }
  }

  Ok(updates)
}

/// Determine if we should update internal dependencies based on config.
fn should_update_dependency(config: &InternalDepUpdate, bump_type: &BumpType) -> bool {
  match config {
    InternalDepUpdate::Always => true,
    InternalDepUpdate::Never => false,
    InternalDepUpdate::Patch => true,
    InternalDepUpdate::Minor => bump_type.priority() >= BumpType::Minor.priority(),
    InternalDepUpdate::Major => bump_type.priority() >= BumpType::Major.priority(),
  }
}

/// Apply the release plan: write package.json files, update changelogs, remove release files.
/// If `dry_run` is true, only print what would happen without writing.
pub fn apply_release_plan(
  workspace: &Workspace,
  plan: &ReleasePlan,
  config: &OxrlsConfig,
  release_dir: &Path,
  dry_run: bool,
  archive: bool,
) -> Result<()> {
  if dry_run {
    print_plan(plan);
    return Ok(());
  }

  // Phase 1: Update package.json files
  for (_name, bump) in &plan.bumps {
    let pkg = workspace
      .packages
      .get(&bump.package_name)
      .ok_or_else(|| OxrlsError::Bump(format!("Package '{}' not found", bump.package_name)))?;

    let pkg_path = pkg.dir.join("package.json");
    let mut pkg_json = PackageJson::read(&pkg_path)?;
    pkg_json.set_version(&bump.new_version);
    PackageJson::write(&pkg_path, &pkg_json)?;

    println!(
      "  {} {} -> {}",
      bump.package_name, bump.old_version, bump.new_version
    );

    // Optionally sync version to Cargo.toml
    if config.sync_cargo_toml {
      let cargo_path = pkg.dir.join("Cargo.toml");
      if cargo_path.exists() {
        let cargo_content = std::fs::read_to_string(&cargo_path)
          .map_err(|e| OxrlsError::Bump(format!("Failed to read Cargo.toml: {}", e)))?;
        // Replace version = "..." in the package section (only the first occurrence)
        let new_cargo = cargo_content.replacen(
          &format!("version = \"{}\"", bump.old_version),
          &format!("version = \"{}\"", bump.new_version),
          1,
        );
        std::fs::write(&cargo_path, new_cargo)
          .map_err(|e| OxrlsError::Bump(format!("Failed to write Cargo.toml: {}", e)))?;
        println!("    Cargo.toml version synced");
      }
    }
  }

  // Phase 2: Update internal dependency ranges
  for update in &plan.internal_dep_updates {
    let mut pkg_json = PackageJson::read(&update.dependent_package_path)?;
    let field = match update.dep_type.as_str() {
      "dependencies" => &mut pkg_json.dependencies,
      "devDependencies" => &mut pkg_json.dev_dependencies,
      "peerDependencies" => &mut pkg_json.peer_dependencies,
      "optionalDependencies" => &mut pkg_json.optional_dependencies,
      _ => continue,
    };

    if let Some(deps) = field
      && let Some(current_range) = deps.get(&update.dep_name).cloned() {
        // Find the old and new versions for this dependency
        if let Some(bump) = plan.bumps.get(&update.dep_name) {
          let new_range = crate::package_json::compute_new_range(
            &current_range,
            &bump.old_version,
            &bump.new_version,
          );
          if new_range != current_range {
            deps.insert(update.dep_name.clone(), new_range.clone());
            println!(
              "  {} {} ({}: {} -> {})",
              update.dependent_package_name,
              update.dep_name,
              update.dep_type,
              current_range,
              new_range
            );
          }
        }
      }

    PackageJson::write(&update.dependent_package_path, &pkg_json)?;
  }

  // Phase 3: Update changelogs
  let is_solo_repo = workspace.packages.len() <= 1;
  let changelog_mode = config.changelog_mode(is_solo_repo);

  if changelog_mode.per_package {
    for (_name, bump) in &plan.bumps {
      let pkg = workspace
        .packages
        .get(&bump.package_name)
        .ok_or_else(|| OxrlsError::Bump(format!("Package '{}' not found", bump.package_name)))?;

      let changelog_path = pkg.dir.join("CHANGELOG.md");

      // Group summaries by type for this package
      let mut type_summaries: IndexMap<BumpType, Vec<String>> = IndexMap::new();
      for rf_path in &bump.release_files {
        if let Ok(rf) = parse_release_file(rf_path)
          && let Some(bt) = rf.releases.get(&bump.package_name) {
            type_summaries
              .entry(*bt)
              .or_default()
              .push(rf.summary.clone());
          }
      }

      if type_summaries.is_empty() {
        type_summaries.insert(bump.bump_type, bump.summaries.clone());
      }

      let entry = ChangelogEntry {
        package_name: bump.package_name.clone(),
        version: bump.new_version.to_string(),
        changes: type_summaries,
      };

      let section = generate_changelog_section(&entry);
      update_changelog(&changelog_path, &section)?;
    }
  }

  if changelog_mode.global {
    // Collect all bumped packages with their summaries for the global changelog
    let global_packages: Vec<(String, semver::Version, BumpType, Vec<String>)> = plan
      .bumps
      .values()
      .map(|bump| {
        (
          bump.package_name.clone(),
          bump.new_version.clone(),
          bump.bump_type,
          bump.summaries.clone(),
        )
      })
      .collect();

    let global_section = generate_global_changelog_section(&global_packages);
    if !global_section.is_empty() {
      let global_changelog_path = workspace.root.join("CHANGELOG.md");
      update_global_changelog(&global_changelog_path, &global_section)?;
    }
  }

  // Save the release manifest for `oxrls release`
  let manifest = ReleaseManifest::from_bumps(&plan.bumps);
  manifest.save(release_dir)?;

  // Phase 4: Consume release files
  // Build a set of package names that are in pre-release mode
  let mut pre_release_pkgs: HashSet<String> = HashSet::new();
  for (_name, bump) in &plan.bumps {
    if !bump.new_version.pre.as_str().is_empty() {
      pre_release_pkgs.insert(bump.package_name.clone());
    }
  }

  // Collect release files that touch at least one pre-release package
  let mut pre_release_files: HashSet<PathBuf> = HashSet::new();
  for (_name, bump) in &plan.bumps {
    if pre_release_pkgs.contains(&bump.package_name) {
      for rf_path in &bump.release_files {
        pre_release_files.insert(rf_path.clone());
      }
    }
  }

  // Closure to check if a package name is in pre-release mode
  let is_pre = |name: &str| -> bool { pre_release_pkgs.contains(name) };

  if archive {
    let archive_dir = release_dir.join("archive");
    for (_name, bump) in &plan.bumps {
      for rf_path in &bump.release_files {
        if pre_release_files.contains(rf_path) {
          crate::release_file::strip_stable_entries(rf_path, is_pre)?;
          println!("  {} (consumed — pre-release entries already in changelog)", rf_path.display());
          continue;
        }
        crate::release_file::archive_release_file(rf_path, &archive_dir)?;
        println!("  {} (archived)", rf_path.display());
      }
    }
  } else {
    let mut consumed: HashSet<PathBuf> = HashSet::new();
    for (_name, bump) in &plan.bumps {
      for rf_path in &bump.release_files {
        if pre_release_files.contains(rf_path) {
          if consumed.insert(rf_path.clone()) {
            crate::release_file::strip_stable_entries(rf_path, is_pre)?;
            println!("  {} (consumed — pre-release entries already in changelog)", rf_path.display());
          }
          continue;
        }
        if consumed.insert(rf_path.clone()) {
          consume_release_file(rf_path)?;
          println!("  {}", rf_path.display());
        }
      }
    }
  }

  Ok(())
}

/// Print the release plan without making changes.
pub fn print_plan(plan: &ReleasePlan) {
  let max_name = plan.bumps.values().map(|b| b.package_name.len()).max().unwrap_or(20);
  println!("Bumped packages:");
  for (_name, bump) in &plan.bumps {
    println!(
      "  {:<width$}  {:>12} →  {:<12}  ({})",
      bump.package_name,
      bump.old_version.to_string(),
      bump.new_version.to_string(),
      bump.bump_type_str(),
      width = max_name
    );
  }

  if !plan.internal_dep_updates.is_empty() {
    println!("\nUpdated internal dependencies:");
    for update in &plan.internal_dep_updates {
      println!(
        "  {} package.json\n    {} {} -> {}",
        update.dependent_package_name,
        update.dep_name,
        update.old_range,
        if update.new_range.is_empty() {
          "(computed during apply)".to_string()
        } else {
          update.new_range.clone()
        }
      );
    }
  }

  println!("\nConsumed release files:");
  let mut seen: HashSet<&PathBuf> = HashSet::new();
  for (_name, bump) in &plan.bumps {
    for rf_path in &bump.release_files {
      if seen.insert(rf_path) {
        println!("  {}", rf_path.display());
      }
    }
  }
}

/// Find all markdown files in the release directory.
pub fn find_release_files(release_dir: &Path) -> Result<Vec<PathBuf>> {
  if !release_dir.exists() {
    return Ok(vec![]);
  }

  let mut files = Vec::new();
  let entries = std::fs::read_dir(release_dir).map_err(OxrlsError::Io)?;

  for entry in entries {
    let entry = entry.map_err(OxrlsError::Io)?;
    let path = entry.path();
    if path.is_file() && path.extension().map(|e| e == "md").unwrap_or(false) {
      // Skip README.md
      if path.file_stem().map(|s| s == "README").unwrap_or(false) {
        continue;
      }
      files.push(path);
    }
  }

  files.sort();
  Ok(files)
}

impl PlannedBump {
  pub fn bump_type_str(&self) -> &str {
    match self.bump_type {
      BumpType::Patch => "patch",
      BumpType::Minor => "minor",
      BumpType::Major => "major",
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::workspace::load_workspace;
  use tempfile::TempDir;

  fn create_test_workspace(tmp: &TempDir) -> Workspace {
    // Root package.json
    let root_pkg = serde_json::json!({
        "name": "root",
        "version": "1.0.0",
        "workspaces": ["packages/*"]
    });
    std::fs::create_dir_all(tmp.path().join("packages/core")).unwrap();
    std::fs::create_dir_all(tmp.path().join("packages/react")).unwrap();
    std::fs::write(
      tmp.path().join("package.json"),
      serde_json::to_string_pretty(&root_pkg).unwrap(),
    )
    .unwrap();

    // Core package
    let core_pkg = serde_json::json!({
        "name": "@scope/core",
        "version": "1.2.3"
    });
    std::fs::write(
      tmp.path().join("packages/core/package.json"),
      serde_json::to_string_pretty(&core_pkg).unwrap(),
    )
    .unwrap();

    // React package with dependency
    let react_pkg = serde_json::json!({
        "name": "@scope/react",
        "version": "1.0.0",
        "dependencies": {
            "@scope/core": "^1.2.3"
        }
    });
    std::fs::write(
      tmp.path().join("packages/react/package.json"),
      serde_json::to_string_pretty(&react_pkg).unwrap(),
    )
    .unwrap();

    load_workspace(tmp.path()).unwrap()
  }

  #[test]
  fn test_resolve_patterns_exact() {
    let tmp = TempDir::new().unwrap();
    let ws = create_test_workspace(&tmp);
    let patterns = vec!["@scope/core".to_string()];
    let resolved = resolve_group_patterns(&patterns, &ws.packages).unwrap();
    assert_eq!(resolved, vec!["@scope/core"]);
  }

  #[test]
  fn test_resolve_patterns_glob() {
    let tmp = TempDir::new().unwrap();
    let ws = create_test_workspace(&tmp);
    let patterns = vec!["@scope/*".to_string()];
    let resolved = resolve_group_patterns(&patterns, &ws.packages).unwrap();
    assert_eq!(resolved.len(), 2);
    assert!(resolved.contains(&"@scope/core".to_string()));
    assert!(resolved.contains(&"@scope/react".to_string()));
  }

  #[test]
  fn test_resolve_patterns_glob_with_negation() {
    let tmp = TempDir::new().unwrap();
    let ws = create_test_workspace(&tmp);
    let patterns = vec![
      "@scope/*".to_string(),
      "!@scope/core".to_string(),
    ];
    let resolved = resolve_group_patterns(&patterns, &ws.packages).unwrap();
    assert_eq!(resolved, vec!["@scope/react"]);
  }

  #[test]
  fn test_fixed_group_with_glob_patterns() {
    let tmp = TempDir::new().unwrap();
    let _ = create_test_workspace(&tmp);
    let workspace = load_workspace(tmp.path()).unwrap();

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    let content = r#"---
"@scope/core": patch
---

Fix bug."#;
    std::fs::write(release_dir.join("test.md"), content).unwrap();

    // Fix all @scope/* packages — both should get the same version
    let config = OxrlsConfig {
      fixed: vec![vec!["@scope/*".to_string()]],
      ..Default::default()
    };

    let plan = build_release_plan(&workspace, &config, &release_dir, false).unwrap();

    let core = plan.bumps.get("@scope/core").unwrap();
    let react = plan.bumps.get("@scope/react").unwrap();

    assert_eq!(core.new_version, react.new_version);
    assert_eq!(core.new_version, semver::Version::new(1, 2, 4));
    assert_eq!(react.new_version, semver::Version::new(1, 2, 4));
  }

  #[test]
  fn test_fixed_group_with_glob_and_negation() {
    let tmp = TempDir::new().unwrap();
    let _ = create_test_workspace(&tmp);
    let workspace = load_workspace(tmp.path()).unwrap();

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    let content = r#"---
"@scope/core": patch
---

Fix bug."#;
    std::fs::write(release_dir.join("test.md"), content).unwrap();

    // Fix all @scope/* EXCEPT react — only core should be affected
    let config = OxrlsConfig {
      fixed: vec![vec![
        "@scope/*".to_string(),
        "!@scope/react".to_string(),
      ]],
      ..Default::default()
    };

    let plan = build_release_plan(&workspace, &config, &release_dir, false).unwrap();

    assert!(plan.bumps.contains_key("@scope/core"));
    assert!(!plan.bumps.contains_key("@scope/react"));
  }

  #[test]
  fn test_build_release_plan() {
    let tmp = TempDir::new().unwrap();
    let workspace = create_test_workspace(&tmp);

    // Create release dir and file
    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    let content = r#"---
"@scope/core": patch
---

Fix transaction mapping bug."#;
    std::fs::write(release_dir.join("calm-blue-fox.md"), content).unwrap();

    let config = OxrlsConfig::default();
    let plan = build_release_plan(&workspace, &config, &release_dir, false).unwrap();

    assert_eq!(plan.bumps.len(), 1);
    let bump = plan.bumps.get("@scope/core").unwrap();
    assert_eq!(bump.old_version, semver::Version::new(1, 2, 3));
    assert_eq!(bump.new_version, semver::Version::new(1, 2, 4));
    assert_eq!(bump.bump_type, BumpType::Patch);
  }

  #[test]
  fn test_build_plan_missing_package() {
    let tmp = TempDir::new().unwrap();
    let workspace = create_test_workspace(&tmp);

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    let content = r#"---
"@scope/missing": patch
---

Fix something."#;
    std::fs::write(release_dir.join("bad.md"), content).unwrap();

    let config = OxrlsConfig::default();
    let result = build_release_plan(&workspace, &config, &release_dir, false);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("@scope/missing"));
  }

  #[test]
  fn test_build_plan_bump_precedence() {
    let tmp = TempDir::new().unwrap();
    let workspace = create_test_workspace(&tmp);

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    // Two release files with different bump types for same package
    let content1 = r#"---
"@scope/core": patch
---

Fix bug."#;
    std::fs::write(release_dir.join("file1.md"), content1).unwrap();

    let content2 = r#"---
"@scope/core": minor
---

Add feature."#;
    std::fs::write(release_dir.join("file2.md"), content2).unwrap();

    let config = OxrlsConfig::default();
    let plan = build_release_plan(&workspace, &config, &release_dir, false).unwrap();

    let bump = plan.bumps.get("@scope/core").unwrap();
    assert_eq!(bump.new_version, semver::Version::new(1, 3, 0)); // minor wins over patch
  }

  #[test]
  fn test_apply_release_plan_dry_run() {
    let tmp = TempDir::new().unwrap();
    let workspace = create_test_workspace(&tmp);

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    let content = r#"---
"@scope/core": patch
---

Fix bug."#;
    std::fs::write(release_dir.join("test.md"), content).unwrap();

    let config = OxrlsConfig::default();
    let plan = build_release_plan(&workspace, &config, &release_dir, false).unwrap();

    // Dry run should not modify files
    apply_release_plan(&workspace, &plan, &config, &release_dir, true, false).unwrap();

    // Check version unchanged
    let core_pkg = PackageJson::read(&tmp.path().join("packages/core/package.json")).unwrap();
    assert_eq!(core_pkg.version.as_deref(), Some("1.2.3"));
  }

  #[test]
  fn test_pre_release_version_in_plan() {
    let tmp = TempDir::new().unwrap();
    let workspace = create_test_workspace(&tmp);

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    let content = r#"---
"@scope/core": patch
---

Fix bug."#;
    std::fs::write(release_dir.join("test.md"), content).unwrap();

    let config = OxrlsConfig {
      pre_mode: vec![crate::config::PreModeEntry {
        tag: "beta".to_string(),
        packages: vec!["@scope/core".to_string()],
      }],
      ..Default::default()
    };

    let plan = build_release_plan(&workspace, &config, &release_dir, false).unwrap();

    let core = plan.bumps.get("@scope/core").unwrap();
    // Should be 1.2.4-beta.1 instead of 1.2.4
    assert_eq!(core.new_version.to_string(), "1.2.3-beta.1");

    // Second bump should increment the counter
    let content2 = r#"---
"@scope/core": patch
---

Fix another bug."#;
    std::fs::write(release_dir.join("test2.md"), content2).unwrap();

    let plan2 = build_release_plan(&workspace, &config, &release_dir, false).unwrap();
    let core2 = plan2.bumps.get("@scope/core").unwrap();
    assert_eq!(core2.new_version.to_string(), "1.2.3-beta.2");
  }

  #[test]
  fn test_pre_release_major_version() {
    let tmp = TempDir::new().unwrap();
    let workspace = create_test_workspace(&tmp);

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    let content = r#"---
"@scope/core": major
---

Breaking change."#;
    std::fs::write(release_dir.join("test.md"), content).unwrap();

    let config = OxrlsConfig {
      pre_mode: vec![crate::config::PreModeEntry {
        tag: "rc".to_string(),
        packages: vec!["@scope/core".to_string()],
      }],
      ..Default::default()
    };

    let plan = build_release_plan(&workspace, &config, &release_dir, false).unwrap();

    let core = plan.bumps.get("@scope/core").unwrap();
    // Major bump from 1.2.3 -> 2.0.0-rc.1
    assert_eq!(core.new_version.to_string(), "1.2.3-rc.1");
    assert_eq!(core.old_version.to_string(), "1.2.3");
  }

  #[test]
  fn test_pre_release_does_not_affect_other_packages() {
    let tmp = TempDir::new().unwrap();
    let workspace = create_test_workspace(&tmp);

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    let content = r#"---
"@scope/core": patch
"@scope/react": minor
---

Multiple changes."#;
    std::fs::write(release_dir.join("test.md"), content).unwrap();

    let config = OxrlsConfig {
      pre_mode: vec![crate::config::PreModeEntry {
        tag: "beta".to_string(),
        packages: vec!["@scope/core".to_string()],
      }],
      ..Default::default()
    };

    let plan = build_release_plan(&workspace, &config, &release_dir, false).unwrap();

    let core = plan.bumps.get("@scope/core").unwrap();
    assert_eq!(core.new_version.to_string(), "1.2.3-beta.1");

    let react = plan.bumps.get("@scope/react").unwrap();
    assert_eq!(react.new_version.to_string(), "1.1.0"); // no pre-release
  }

  #[test]
  fn test_fixed_group_constraint() {
    let tmp = TempDir::new().unwrap();
    let _workspace = create_test_workspace(&tmp);

    // Add a third package in a fixed group with core
    let utils_pkg = serde_json::json!({
        "name": "@scope/utils",
        "version": "0.5.0"
    });
    std::fs::create_dir_all(tmp.path().join("packages/utils")).unwrap();
    std::fs::write(
      tmp.path().join("packages/utils/package.json"),
      serde_json::to_string_pretty(&utils_pkg).unwrap(),
    )
    .unwrap();

    // Reload workspace to pick up the new package
    let workspace = load_workspace(tmp.path()).unwrap();

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    let content = r#"---
"@scope/core": patch
---

Fix bug."#;
    std::fs::write(release_dir.join("test.md"), content).unwrap();

    let config = OxrlsConfig {
      fixed: vec![vec!["@scope/core".to_string(), "@scope/utils".to_string()]],
      ..Default::default()
    };

    let plan = build_release_plan(&workspace, &config, &release_dir, false).unwrap();

    // Both packages should be bumped to the same version (based on highest old version)
    let core = plan.bumps.get("@scope/core").unwrap();
    let utils = plan.bumps.get("@scope/utils").unwrap();

    assert_eq!(core.new_version, utils.new_version);
    // Highest old version is @scope/core 1.2.3, patched -> 1.2.4
    assert_eq!(core.new_version, semver::Version::new(1, 2, 4));
    assert_eq!(utils.new_version, semver::Version::new(1, 2, 4));
  }

  #[test]
  fn test_fixed_group_uses_highest_old_version() {
    let tmp = TempDir::new().unwrap();
    let _workspace = create_test_workspace(&tmp);

    // Add a package with a higher version
    let utils_pkg = serde_json::json!({
        "name": "@scope/utils",
        "version": "2.0.0"
    });
    std::fs::create_dir_all(tmp.path().join("packages/utils")).unwrap();
    std::fs::write(
      tmp.path().join("packages/utils/package.json"),
      serde_json::to_string_pretty(&utils_pkg).unwrap(),
    )
    .unwrap();

    // Reload workspace to pick up the new package
    let workspace = load_workspace(tmp.path()).unwrap();

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    let content = r#"---
"@scope/utils": major
---

Breaking change."#;
    std::fs::write(release_dir.join("test.md"), content).unwrap();

    let config = OxrlsConfig {
      fixed: vec![vec!["@scope/core".to_string(), "@scope/utils".to_string()]],
      ..Default::default()
    };

    let plan = build_release_plan(&workspace, &config, &release_dir, false).unwrap();

    let core = plan.bumps.get("@scope/core").unwrap();
    let utils = plan.bumps.get("@scope/utils").unwrap();

    // Both should be 3.0.0 (highest old version 2.0.0 + major bump = 3.0.0)
    assert_eq!(core.new_version, semver::Version::new(3, 0, 0));
    assert_eq!(utils.new_version, semver::Version::new(3, 0, 0));
  }

  #[test]
  fn test_linked_group_shares_bump_type() {
    let tmp = TempDir::new().unwrap();
    let workspace = create_test_workspace(&tmp);

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    // Core gets patch, react gets minor — linked group means both get the max (minor)
    let content1 = r#"---
"@scope/core": patch
---

Fix bug."#;
    std::fs::write(release_dir.join("f1.md"), content1).unwrap();

    let content2 = r#"---
"@scope/react": minor
---

Add feature."#;
    std::fs::write(release_dir.join("f2.md"), content2).unwrap();

    let config = OxrlsConfig {
      linked: vec![vec!["@scope/core".to_string(), "@scope/react".to_string()]],
      ..Default::default()
    };

    let plan = build_release_plan(&workspace, &config, &release_dir, false).unwrap();

    let core = plan.bumps.get("@scope/core").unwrap();
    let react = plan.bumps.get("@scope/react").unwrap();

    // Both should be minor bumps
    assert_eq!(core.bump_type, BumpType::Minor);
    assert_eq!(react.bump_type, BumpType::Minor);
    // Core: 1.2.3 -> 1.3.0, React: 1.0.0 -> 1.1.0
    assert_eq!(core.new_version, semver::Version::new(1, 3, 0));
    assert_eq!(react.new_version, semver::Version::new(1, 1, 0));
  }

  #[test]
  fn test_internal_dependency_updates() {
    let tmp = TempDir::new().unwrap();
    let workspace = create_test_workspace(&tmp);

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    let content = r#"---
"@scope/core": patch
---

Fix bug."#;
    std::fs::write(release_dir.join("test.md"), content).unwrap();

    let config = OxrlsConfig::default();
    let plan = build_release_plan(&workspace, &config, &release_dir, false).unwrap();

    // Check that @scope/react's dependency on @scope/core is flagged for update
    let has_core_update = plan
      .internal_dep_updates
      .iter()
      .any(|u| u.dep_name == "@scope/core" && u.dependent_package_name == "@scope/react");
    assert!(has_core_update);
  }

  #[test]
  fn test_fixed_group_summary_does_not_chain() {
    // When a fixed group pulls in multiple packages, the generated summary
    // for each package should only list the directly-bumped ones, not a growing chain.
    let tmp = TempDir::new().unwrap();
    let _workspace = create_test_workspace(&tmp);

    // Add two more packages to the fixed group
    let pkg_a = serde_json::json!({ "name": "@scope/utils", "version": "0.1.0" });
    let pkg_b = serde_json::json!({ "name": "@scope/tools", "version": "0.1.0" });
    std::fs::create_dir_all(tmp.path().join("packages/utils")).unwrap();
    std::fs::create_dir_all(tmp.path().join("packages/tools")).unwrap();
    std::fs::write(
      tmp.path().join("packages/utils/package.json"),
      serde_json::to_string_pretty(&pkg_a).unwrap(),
    )
    .unwrap();
    std::fs::write(
      tmp.path().join("packages/tools/package.json"),
      serde_json::to_string_pretty(&pkg_b).unwrap(),
    )
    .unwrap();

    let workspace = load_workspace(tmp.path()).unwrap();

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    // Only core has a release file — utils and tools are pulled in by the fixed group
    let content = r#"---
"@scope/core": minor
---

Completely rewritten core logic."#;
    std::fs::write(release_dir.join("test.md"), content).unwrap();

    let config = OxrlsConfig {
      fixed: vec![vec![
        "@scope/core".to_string(),
        "@scope/utils".to_string(),
        "@scope/tools".to_string(),
      ]],
      ..Default::default()
    };

    let plan = build_release_plan(&workspace, &config, &release_dir, false).unwrap();

    let utils = plan.bumps.get("@scope/utils").unwrap();
    let tools = plan.bumps.get("@scope/tools").unwrap();

    // Both should list ONLY @scope/core, not a growing chain
    assert_eq!(
      utils.summaries,
      vec!["Updated with @scope/core.".to_string()]
    );
    assert_eq!(
      tools.summaries,
      vec!["Updated with @scope/core.".to_string()]
    );
  }

  #[test]
  fn test_mixed_release_file_strips_stable_entries() {
    // When a release file mentions both a pre-release and a stable package,
    // the bump should strip the stable entries from the file so they don't
    // repeat on the next bump, while keeping the pre-release entries.
    let tmp = TempDir::new().unwrap();
    let workspace = create_test_workspace(&tmp);

    let release_dir = tmp.path().join(".oxrls");
    std::fs::create_dir_all(&release_dir).unwrap();

    // Create a release file that references both a pre-release and a stable package
    let content = r#"---
"@scope/core": patch
"@scope/react": minor
---

Mixed changes for pre-release and stable."#;
    let rf_path = release_dir.join("mixed.md");
    std::fs::write(&rf_path, content).unwrap();

    let config = OxrlsConfig {
      pre_mode: vec![crate::config::PreModeEntry {
        tag: "beta".to_string(),
        packages: vec!["@scope/core".to_string()],
      }],
      ..Default::default()
    };

    let plan = build_release_plan(&workspace, &config, &release_dir, false).unwrap();

    // Verify both packages are in the plan
    assert!(plan.bumps.contains_key("@scope/core"));
    assert!(plan.bumps.contains_key("@scope/react"));

    // Core is in pre-mode -> should have a pre-release version
    assert!(plan.bumps.get("@scope/core").unwrap().new_version.to_string().contains("beta"));
    // React is not in pre-mode -> should have a normal version
    assert!(!plan.bumps.get("@scope/react").unwrap().new_version.to_string().contains("beta"));

    // Apply the release plan
    apply_release_plan(&workspace, &plan, &config, &release_dir, false, false).unwrap();

    // The release file should be consumed — changelog already captured the content
    assert!(!rf_path.exists(), "Release file should be consumed as pre-release entries were already captured");
  }
}