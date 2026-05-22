use std::path::{Path, PathBuf};

use clap::Parser;
use indexmap::IndexMap;
use inquire::{MultiSelect, Select, Text};

use vp_release::bump::{apply_release_plan, build_release_plan, find_release_files, print_plan};
use vp_release::cli::{Cli, Commands};
use vp_release::config::OxrlsConfig;
use vp_release::error::Result;
use vp_release::release_file::{create_release_file, parse_release_file, BumpType};
use vp_release::workspace::{find_workspace_root, load_workspace};

fn main() {
  let cli = Cli::parse();

  let result = match &cli.command {
    Commands::Init { force, release_dir } => cmd_init(*force, release_dir.as_deref()),
    Commands::New {
      packages,
      summary,
      details,
    } => cmd_new(packages, summary.as_deref(), details.as_deref()),
    Commands::Status => cmd_status(),
    Commands::Bump { dry_run, archive } => cmd_bump(*dry_run, *archive),
  };

  if let Err(e) = result {
    eprintln!("Error: {}", e);
    std::process::exit(1);
  }
}

fn cmd_init(force: bool, release_dir: Option<&str>) -> Result<()> {
  let cwd = std::env::current_dir().map_err(vp_release::error::OxrlsError::Io)?;

  // Determine config path
  let config_path = cwd.join("oxrls.json");

  let mut config = OxrlsConfig::default();
  if let Some(dir) = release_dir {
    config.release_dir = dir.to_string();
  }

  // Write config
  OxrlsConfig::write_to(&config_path, &config, force)?;
  println!("Created config file: {}", config_path.display());

  // Create release directory with README
  let release_dir = cwd.join(&config.release_dir);
  std::fs::create_dir_all(&release_dir).map_err(|e| {
    vp_release::error::OxrlsError::Config(format!("Failed to create release dir: {}", e))
  })?;

  let readme_path = release_dir.join("README.md");
  if !readme_path.exists() {
    let readme_content = format!(
      "# {} Release Files\n\nThis directory contains pending release files.\n\
            Use `oxrls new` to create a release file and `oxrls bump` to apply them.\n",
      config.release_dir.trim_start_matches('.')
    );
    std::fs::write(&readme_path, readme_content).map_err(|e| {
      vp_release::error::OxrlsError::Config(format!("Failed to create README: {}", e))
    })?;
    println!("Created release directory: {}", release_dir.display());
  }

  println!("\noxrls is ready! Use `oxrls new` to create a release file.");
  Ok(())
}

fn get_release_dir(root: &Path, config: &OxrlsConfig, config_path: &Path) -> PathBuf {
  if !config_path.as_os_str().is_empty() {
    config.release_dir_abs(config_path)
  } else {
    root.join(&config.release_dir)
  }
}

fn cmd_new(packages: &[String], summary: Option<&str>, details: Option<&str>) -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;
  let workspace = load_workspace(&root)?;

  let (config, config_path) = OxrlsConfig::load(&root)?;
  let release_dir = get_release_dir(&root, &config, &config_path);

  let releases: IndexMap<String, BumpType>;

  if packages.is_empty() {
    // Interactive mode
    if workspace.packages.is_empty() {
      return Err(vp_release::error::OxrlsError::ReleaseFile(
        "No workspace packages found. Run `oxrls init` first or add packages.".to_string(),
      ));
    }

    let package_names: Vec<&String> = workspace.packages.keys().collect();

    let selected = MultiSelect::new("Which packages changed?", package_names)
      .prompt()
      .map_err(|e| vp_release::error::OxrlsError::Other(format!("Selection failed: {}", e)))?;

    if selected.is_empty() {
      return Err(vp_release::error::OxrlsError::ReleaseFile(
        "No packages selected.".to_string(),
      ));
    }

    let mut releases_map: IndexMap<String, BumpType> = IndexMap::new();
    for pkg_name in selected {
      let bump_options = vec!["patch", "minor", "major"];
      let bump = Select::new(&format!("Bump type for \"{}\":", pkg_name), bump_options)
        .prompt()
        .map_err(|e| vp_release::error::OxrlsError::Other(format!("Selection failed: {}", e)))?;

      releases_map.insert(pkg_name.clone(), bump.parse::<BumpType>()?);
    }

    let summary_text = Text::new("Summary of the change:")
      .prompt()
      .map_err(|e| vp_release::error::OxrlsError::Other(format!("Input failed: {}", e)))?;

    let details_text: Option<String> = Text::new("Optional details (enter to skip):")
      .prompt()
      .ok()
      .filter(|s: &String| !s.is_empty());

    releases = releases_map;
    let path = create_release_file(
      &release_dir,
      &releases,
      &summary_text,
      details_text.as_deref(),
    )?;
    println!("\nCreated release file: {}", path.display());
  } else {
    // Non-interactive mode: parse --package args
    let mut releases_map: IndexMap<String, BumpType> = IndexMap::new();
    for pkg_arg in packages {
      let parts: Vec<&str> = pkg_arg.splitn(2, ':').collect();
      let pkg_name = parts[0];
      let bump_str = parts.get(1).ok_or_else(|| {
        vp_release::error::OxrlsError::ReleaseFile(format!(
          "Invalid package format: \"{}\". Expected format: \"@scope/pkg:bumptype\"",
          pkg_arg
        ))
      })?;
      let bump_type = bump_str.parse::<BumpType>()?;
      releases_map.insert(pkg_name.to_string(), bump_type);
    }

    let summary_text = summary.unwrap_or("No summary provided.");

    releases = releases_map;
    let path = create_release_file(&release_dir, &releases, summary_text, details)?;
    println!("Created release file: {}", path.display());
  }

  Ok(())
}

fn cmd_status() -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;
  let workspace = load_workspace(&root)?;

  let (config, config_path) = OxrlsConfig::load(&root)?;
  let release_dir = get_release_dir(&root, &config, &config_path);

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
        println!("  {}", rf.path.display());
        for (pkg, bump) in &rf.releases {
          println!("    {}  {}", pkg, bump);
        }
        println!();
      }
      Err(e) => {
        eprintln!("  ERROR parsing {}: {}", file_path.display(), e);
      }
    }
  }

  // Show calculated bumps
  match build_release_plan(&workspace, &config, &release_dir) {
    Ok(plan) => {
      println!("Calculated bumps:\n");
      for (_name, bump) in &plan.bumps {
        println!(
          "  {} {} -> {}",
          bump.package_name, bump.old_version, bump.new_version
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
      std::process::exit(1);
    }
  }

  Ok(())
}

fn cmd_bump(dry_run: bool, archive: bool) -> Result<()> {
  let root = find_workspace_root(Path::new("."))?;
  let workspace = load_workspace(&root)?;

  let (config, config_path) = OxrlsConfig::load(&root)?;
  let release_dir = get_release_dir(&root, &config, &config_path);

  let plan = build_release_plan(&workspace, &config, &release_dir)?;

  if dry_run {
    println!("[DRY RUN] Would apply the following release plan:\n");
    print_plan(&plan);
    return Ok(());
  }

  println!("Bumped packages:\n");
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
  let mut seen: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
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
