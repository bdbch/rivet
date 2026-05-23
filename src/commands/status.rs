//! `oxrls status` — display pending release files and calculated bumps.
//!
//! Lists all unreleased release files and shows what version bumps
//! would be applied by `oxrls bump`.

use std::path::Path;

use crate::bump::{build_release_plan, find_release_files};
use crate::config::OxrlsConfig;
use crate::error::Result;
use crate::release_file::parse_release_file;
use crate::workspace::{find_workspace_root, load_workspace};

pub fn cmd_status() -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;
  let workspace = load_workspace(&root)?;

  let (config, config_path) = OxrlsConfig::load(&root)?;
  let release_dir = crate::get_release_dir(&root, &config, &config_path);

  let files = find_release_files(&release_dir)?;

  if files.is_empty() {
    println!(
      "No pending release files found in {}.\n",
      release_dir.display()
    );
    return Ok(());
  }

  println!("Pending release files:\n");

  for file_path in &files {
    match parse_release_file(file_path) {
      Ok(rf) => {
        let fname = file_path
          .file_name()
          .map(|n| n.to_string_lossy())
          .unwrap_or_default();
        println!("  {}", fname);
        for (pkg, bump) in &rf.releases {
          println!("    {}  {}", pkg, bump);
        }
      }
      Err(e) => {
        eprintln!("  ERROR parsing {}: {}", file_path.display(), e);
      }
    }
  }

  println!();

  match build_release_plan(&workspace, &config, &release_dir) {
    Ok(plan) => {
      let _max_name = plan
        .bumps
        .values()
        .map(|b| b.package_name.len())
        .max()
        .unwrap_or(20);
      println!("Calculated bumps:");
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
        println!("\nInternal dependency updates:\n");
        for update in &plan.internal_dep_updates {
          println!(
            "  {} will update {} ({})",
            update.dependent_package_name, update.dep_name, update.dep_type
          );
        }
      }
    }
    Err(e) => {
      eprintln!("\nCould not calculate bumps: {}", e);
      return Err(e);
    }
  }

  Ok(())
}
