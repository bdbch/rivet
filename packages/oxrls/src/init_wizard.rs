use crate::config::{InternalDepUpdate, OxrlsConfig};
use crate::error::{OxrlsError, Result};
use crate::workspace::Workspace;
use inquire::{Confirm, MultiSelect, Select, Text};

/// Run the interactive configuration wizard.
/// Mutates `config` based on user input.
pub fn run_init_wizard(
  config: &mut OxrlsConfig,
  workspace: &Workspace,
  is_monorepo: bool,
) -> Result<()> {
  println!("── oxrls configuration wizard ──\n");

  // 1. Release directory
  let rd = Text::new("Release directory:")
    .with_default(&config.release_dir)
    .prompt()
    .map_err(|e| OxrlsError::Other(format!("Input failed: {}", e)))?;
  if !rd.trim().is_empty() {
    config.release_dir = rd.trim().to_string();
  }

  // 2. Changelog questions
  let changelog = Confirm::new("Generate changelogs?")
    .with_default(true)
    .prompt()
    .map_err(|e| OxrlsError::Other(format!("Input failed: {}", e)))?;

  if changelog {
    config.changelog = true;

    if is_monorepo {
      config.generate_packages_changelog =
        Confirm::new("  Create a CHANGELOG.md for each package?")
          .with_default(true)
          .prompt()
          .map_err(|e| OxrlsError::Other(format!("Input failed: {}", e)))?;
      config.generate_global_changelog = Confirm::new(
        "  Create a global CHANGELOG.md in the project root (aggregating all changes)?",
      )
      .with_default(false)
      .prompt()
      .map_err(|e| OxrlsError::Other(format!("Input failed: {}", e)))?;
    } else {
      config.generate_packages_changelog = false;
      config.generate_global_changelog = true;
      println!("  A root-level CHANGELOG.md will be created (single-package project).");
    }
  } else {
    config.changelog = false;
    config.generate_packages_changelog = false;
    config.generate_global_changelog = false;
  }

  // 3. Base branch
  let branch = Text::new("Base branch:")
    .with_default(&config.base_branch)
    .prompt()
    .map_err(|e| OxrlsError::Other(format!("Input failed: {}", e)))?;
  if !branch.trim().is_empty() {
    config.base_branch = branch.trim().to_string();
  }

  // 4. Update internal dependencies strategy
  let dep_strategy = Select::new(
    "Update internal dependency ranges when a dependency is bumped:",
    vec!["patch (default)", "minor", "major", "always", "never"],
  )
  .prompt()
  .map_err(|e| OxrlsError::Other(format!("Input failed: {}", e)))?;

  config.update_internal_dependencies = match dep_strategy {
    "patch (default)" | "patch" => InternalDepUpdate::Patch,
    "minor" => InternalDepUpdate::Minor,
    "major" => InternalDepUpdate::Major,
    "always" => InternalDepUpdate::Always,
    "never" => InternalDepUpdate::Never,
    _ => InternalDepUpdate::Patch,
  };

  // 5. Access
  let access = Select::new(
    "Default npm access (can be overridden per-package via publishConfig.access):",
    vec!["public", "restricted"],
  )
  .prompt()
  .map_err(|e| OxrlsError::Other(format!("Input failed: {}", e)))?;
  config.access = match access {
    "restricted" => crate::config::Access::Restricted,
    _ => crate::config::Access::Public,
  };
  // 6. Sync Cargo.toml
  let sync_cargo = Confirm::new(
    "Sync version with Cargo.toml files alongside package.json? (useful for Rust/NAPI projects)",
  )
  .with_default(false)
  .prompt()
  .map_err(|e| OxrlsError::Other(format!("Input failed: {}", e)))?;
  config.sync_cargo_toml = sync_cargo;

  // 7. Linked packages (only if monorepo with 2+ packages)
  if is_monorepo && workspace.packages.len() > 1 {
    loop {
      let want_linked = Confirm::new(
        "\nLinked packages: packages in a linked group share the same bump type (major/minor/patch). \
         If one gets a major bump, all get a major bump — but each keeps its own version number.\n\n\
         Add a linked group?",
      )
      .with_default(false)
      .prompt()
      .map_err(|e| OxrlsError::Other(format!("Input failed: {}", e)))?;

      if !want_linked {
        break;
      }

      let pkg_names: Vec<&String> = workspace.packages.keys().collect();
      let selected = MultiSelect::new("  Select packages for this linked group:", pkg_names)
        .prompt()
        .map_err(|e| OxrlsError::Other(format!("Selection failed: {}", e)))?;

      if !selected.is_empty() {
        let group: Vec<String> = selected.iter().map(|s| (*s).clone()).collect();
        config.linked.push(group);
        println!("  Added linked group.");
      }
    }
  }

  // 8. Fixed packages (only if monorepo with 2+ packages)
  if is_monorepo && workspace.packages.len() > 1 {
    loop {
      let want_fixed = Confirm::new(
        "\nFixed packages: packages in a fixed group always share the same version. \
         If one gets a bump, all bump to the same version.\n\n\
         Add a fixed group?",
      )
      .with_default(false)
      .prompt()
      .map_err(|e| OxrlsError::Other(format!("Input failed: {}", e)))?;

      if !want_fixed {
        break;
      }

      let pkg_names: Vec<&String> = workspace.packages.keys().collect();
      let selected = MultiSelect::new("  Select packages for this fixed group:", pkg_names)
        .prompt()
        .map_err(|e| OxrlsError::Other(format!("Selection failed: {}", e)))?;

      if !selected.is_empty() {
        let group: Vec<String> = selected.iter().map(|s| (*s).clone()).collect();
        config.fixed.push(group);
        println!("  Added fixed group.");
      }
    }
  }

  println!("\n── Configuration complete ──\n");

  Ok(())
}
