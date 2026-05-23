use crate::bump::plan::ReleasePlan;
use crate::changelog::{
  ChangelogEntry, generate_changelog_section, generate_global_changelog_section, update_changelog,
  update_global_changelog,
};
use crate::config::OxrlsConfig;
use crate::error::{OxrlsError, Result};
use crate::package_json::PackageJson;
use crate::release::ReleaseManifest;
use crate::release_file::{BumpType, consume_release_file, parse_release_file};
use crate::workspace::Workspace;
use indexmap::IndexMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

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
      && let Some(current_range) = deps.get(&update.dep_name).cloned()
    {
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
          && let Some(bt) = rf.releases.get(&bump.package_name)
        {
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

  if archive {
    let archive_dir = release_dir.join("archive");
    for (_name, bump) in &plan.bumps {
      for rf_path in &bump.release_files {
        if pre_release_files.contains(rf_path) {
          // Pre-release entries were already recorded in the changelog
          // during Phase 3, so just consume the file to avoid replaying.
          consume_release_file(rf_path)?;
          println!(
            "  {} (consumed — pre-release entries already in changelog)",
            rf_path.display()
          );
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
            // Pre-release entries already recorded in changelog.
            consume_release_file(rf_path)?;
            println!(
              "  {} (consumed — pre-release entries already in changelog)",
              rf_path.display()
            );
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

  // Persist the pre-release counters only after all writes have succeeded.
  // This is critical for atomicity — if an earlier step failed, the counters
  // remain unchanged and a retry produces the same pre-release versions.
  plan.pre_state.save(release_dir)?;

  Ok(())
}

/// Print the release plan without making changes.
pub fn print_plan(plan: &ReleasePlan) {
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
