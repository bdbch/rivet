//! `oxrls bump` — apply pending release files to workspace packages.
//!
//! Builds a release plan from all pending release files, then applies
//! version bumps and consumes the files. Supports `--dry-run` and `--archive`.

use std::collections::HashSet;
use std::path::Path;

use crate::bump::{apply_release_plan, build_release_plan, print_plan};
use crate::config::OxrlsConfig;
use crate::error::Result;
use crate::workspace::{find_workspace_root, load_workspace};

pub fn cmd_bump(dry_run: bool, archive: bool) -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;
  let workspace = load_workspace(&root)?;

  let (config, config_path) = OxrlsConfig::load(&root)?;
  let release_dir = crate::get_release_dir(&root, &config, &config_path);

  let plan = build_release_plan(&workspace, &config, &release_dir)?;

  if dry_run {
    println!("[DRY RUN] Would apply the following release plan:\n");
    print_plan(&plan);
    return Ok(());
  }

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
  let plan_clone = plan.clone();
  apply_release_plan(&workspace, &plan, &config, &release_dir, false, archive)?;

  if !plan_clone.internal_dep_updates.is_empty() {
    println!("\nUpdated internal dependencies:");
    for update in &plan_clone.internal_dep_updates {
      println!(
        "  {} {}",
        update.dependent_package_path.display(),
        update.dep_name
      );
    }
  }

  println!("\nConsumed release files:");
  let mut seen: HashSet<std::path::PathBuf> = HashSet::new();
  for (_name, bump) in &plan_clone.bumps {
    for rf_path in &bump.release_files {
      if seen.insert(rf_path.clone()) {
        println!("  {}", rf_path.display());
      }
    }
  }

  println!("\nDone!");
  Ok(())
}
