//! Megatest: Full user flow from init through bump with pre-mode, linked, fixed packages.
//!
//! Workspace: 8 packages (independent, linked, fixed, pre-mode).
//! Phases: scaffold → config → release files → plan → dry-run → bump → deps → counter.

use indexmap::IndexMap;
use oxrls::bump::{apply_release_plan, build_release_plan, find_release_files, print_plan};
use oxrls::config::{Access, InternalDepUpdate, OxrlsConfig, PreModeEntry};
use oxrls::release_file::{BumpType, create_release_file};
use oxrls::workspace::PackageJson;
use oxrls::workspace::load_workspace;
use semver::Version;
use std::path::Path;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_pkg(dir: &Path, name: &str, version: &str, deps: Option<&[(&str, &str)]>) {
  let mut map = serde_json::Map::new();
  map.insert(
    "name".to_string(),
    serde_json::Value::String(name.to_string()),
  );
  map.insert(
    "version".to_string(),
    serde_json::Value::String(version.to_string()),
  );
  if let Some(d) = deps {
    let dep_map: serde_json::Map<String, serde_json::Value> = d
      .iter()
      .map(|(k, v)| (k.to_string(), serde_json::Value::String(v.to_string())))
      .collect();
    map.insert(
      "dependencies".to_string(),
      serde_json::Value::Object(dep_map),
    );
  }
  std::fs::create_dir_all(dir).unwrap();
  std::fs::write(
    dir.join("package.json"),
    serde_json::to_string_pretty(&map).unwrap(),
  )
  .unwrap();
}

fn scaffold_workspace(tmp: &TempDir) {
  let root = tmp.path();
  let mut root_map = serde_json::Map::new();
  root_map.insert(
    "name".to_string(),
    serde_json::Value::String("root".to_string()),
  );
  root_map.insert(
    "version".to_string(),
    serde_json::Value::String("1.0.0".to_string()),
  );
  root_map.insert("private".to_string(), serde_json::Value::Bool(true));
  root_map.insert(
    "workspaces".to_string(),
    serde_json::Value::Array(
      ["packages/*"]
        .iter()
        .map(|s| serde_json::Value::String(s.to_string()))
        .collect(),
    ),
  );
  std::fs::write(
    root.join("package.json"),
    serde_json::to_string_pretty(&root_map).unwrap(),
  )
  .unwrap();
  write_pkg(&root.join("packages/core"), "@scope/core", "1.0.0", None);
  write_pkg(
    &root.join("packages/utils"),
    "@scope/utils",
    "1.0.0",
    Some(&[("@scope/core", "^1.0.0")]),
  );
  write_pkg(&root.join("packages/ui"), "@scope/ui", "2.0.0", None);
  write_pkg(&root.join("packages/icons"), "@scope/icons", "2.0.0", None);
  write_pkg(&root.join("packages/app"), "@scope/app", "3.0.0", None);
  write_pkg(&root.join("packages/web"), "@scope/web", "3.0.0", None);
  write_pkg(
    &root.join("packages/internal"),
    "@scope/internal",
    "1.0.0",
    None,
  );
  write_pkg(&root.join("packages/tools"), "@scope/tools", "1.5.0", None);
}

fn create_config(release_dir: &Path) -> OxrlsConfig {
  let config_path = release_dir.join("config.json");
  let config = OxrlsConfig {
    changelog: false,
    generate_packages_changelog: false,
    generate_global_changelog: false,
    update_internal_dependencies: InternalDepUpdate::Patch,
    base_branch: "main".to_string(),
    access: Access::Public,
    fixed: vec![vec!["@scope/app".to_string(), "@scope/web".to_string()]],
    linked: vec![vec!["@scope/ui".to_string(), "@scope/icons".to_string()]],
    pre_mode: vec![
      PreModeEntry {
        tag: "alpha".to_string(),
        packages: vec!["@scope/internal".to_string()],
      },
      PreModeEntry {
        tag: "beta".to_string(),
        packages: vec!["@scope/tools".to_string()],
      },
    ],
    ..OxrlsConfig::default()
  };
  OxrlsConfig::write_to(&config_path, &config, true).unwrap();
  config
}

/// Create release files. Note: @scope/icons gets a PATCH bump so the linked
/// group elevates it to MINOR (matching @scope/ui's higher bump type).
fn create_releases(release_dir: &Path) {
  std::fs::create_dir_all(release_dir).unwrap();

  let mut r1 = IndexMap::new();
  r1.insert("@scope/core".to_string(), BumpType::Patch);
  create_release_file(release_dir, &r1, "Fix core transaction bug", None).unwrap();

  let mut r2 = IndexMap::new();
  r2.insert("@scope/ui".to_string(), BumpType::Minor);
  create_release_file(release_dir, &r2, "Add Button component", None).unwrap();

  // @scope/icons gets Patch — linked group will bump it to Minor
  let mut r2b = IndexMap::new();
  r2b.insert("@scope/icons".to_string(), BumpType::Patch);
  create_release_file(release_dir, &r2b, "Fix icon alignment", None).unwrap();

  let mut r3 = IndexMap::new();
  r3.insert("@scope/app".to_string(), BumpType::Major);
  create_release_file(release_dir, &r3, "Breaking: Redesign API", None).unwrap();

  let mut r4 = IndexMap::new();
  r4.insert("@scope/internal".to_string(), BumpType::Minor);
  create_release_file(release_dir, &r4, "Internal refactor", None).unwrap();

  let mut r5 = IndexMap::new();
  r5.insert("@scope/tools".to_string(), BumpType::Patch);
  create_release_file(release_dir, &r5, "Fix tools config", None).unwrap();
}

fn assert_version(dir: &Path, expected: &str) {
  let pkg = PackageJson::read(&dir.join("package.json")).unwrap();
  assert_eq!(
    pkg.version.as_deref(),
    Some(expected),
    "{} has wrong version",
    dir.display()
  );
}

fn assert_not_bumped(plan: &oxrls::bump::ReleasePlan, pkg_name: &str) {
  assert!(
    !plan.bumps.contains_key(pkg_name),
    "{pkg_name} should NOT be bumped"
  );
}

// ---------------------------------------------------------------------------
// Megatest
// ---------------------------------------------------------------------------

#[test]
fn test_full_user_flow_init_pre_mode_bump() {
  let tmp = TempDir::new().unwrap();

  // ── Phase 1: Scaffold workspace ──
  scaffold_workspace(&tmp);
  let workspace = load_workspace(tmp.path()).unwrap();
  assert_eq!(workspace.packages.len(), 8);
  for name in &[
    "@scope/core",
    "@scope/utils",
    "@scope/ui",
    "@scope/icons",
    "@scope/app",
    "@scope/web",
    "@scope/internal",
    "@scope/tools",
  ] {
    assert!(workspace.packages.contains_key(*name), "Missing: {name}");
  }

  // ── Phase 2: Config with linked, fixed, pre-mode ──
  let oxrls_dir = tmp.path().join(".oxrls");
  let config = create_config(&oxrls_dir);
  assert_eq!(config.linked[0].len(), 2);
  assert_eq!(config.fixed.len(), 1);
  assert_eq!(config.pre_mode.len(), 2);

  // ── Phase 3: Create release files ──
  create_releases(&oxrls_dir);
  assert_eq!(find_release_files(&oxrls_dir).unwrap().len(), 6);

  // ── Phase 4: Build plan & verify every bump ──
  println!("\n--- Status (before bump) ---");
  let plan = build_release_plan(&workspace, &config, &oxrls_dir).unwrap();
  print_plan(&plan);

  assert_eq!(plan.bumps.len(), 7, "Expected 7 bumps");

  // @scope/core: patch
  let core = plan.bumps.get("@scope/core").unwrap();
  assert_eq!(core.old_version, Version::new(1, 0, 0));
  assert_eq!(core.new_version, Version::new(1, 0, 1));
  assert_eq!(core.bump_type, BumpType::Patch);

  // @scope/ui: minor
  let ui = plan.bumps.get("@scope/ui").unwrap();
  assert_eq!(ui.old_version, Version::new(2, 0, 0));
  assert_eq!(ui.new_version, Version::new(2, 1, 0));
  assert_eq!(ui.bump_type, BumpType::Minor);

  // @scope/icons: linked, was patch → elevated to minor
  let icons = plan.bumps.get("@scope/icons").unwrap();
  assert_eq!(icons.old_version, Version::new(2, 0, 0));
  assert_eq!(icons.new_version, Version::new(2, 1, 0));
  assert_eq!(
    icons.bump_type,
    BumpType::Minor,
    "Linked group should elevate patch→minor"
  );

  // @scope/app: major (fixed group anchor)
  let app = plan.bumps.get("@scope/app").unwrap();
  assert_eq!(app.old_version, Version::new(3, 0, 0));
  assert_eq!(app.new_version, Version::new(4, 0, 0));
  assert_eq!(app.bump_type, BumpType::Major);

  // @scope/web: fixed group — added automatically, shares @scope/app's version
  let web = plan.bumps.get("@scope/web").unwrap();
  assert_eq!(web.old_version, Version::new(3, 0, 0));
  assert_eq!(web.new_version, Version::new(4, 0, 0));
  assert_eq!(
    web.new_version, app.new_version,
    "Fixed group must share version"
  );

  // @scope/internal: pre-mode alpha, first bump → 1.1.0-alpha.1
  let internal = plan.bumps.get("@scope/internal").unwrap();
  assert_eq!(internal.old_version, Version::new(1, 0, 0));
  assert_eq!(
    internal.new_version,
    Version::parse("1.1.0-alpha.1").unwrap()
  );
  assert_eq!(internal.bump_type, BumpType::Minor);

  // @scope/tools: pre-mode beta, first bump → 1.5.1-beta.1
  let tools = plan.bumps.get("@scope/tools").unwrap();
  assert_eq!(tools.old_version, Version::new(1, 5, 0));
  assert_eq!(tools.new_version, Version::parse("1.5.1-beta.1").unwrap());
  assert_eq!(tools.bump_type, BumpType::Patch);

  // @scope/utils: NOT bumped
  assert_not_bumped(&plan, "@scope/utils");

  // ── Phase 5: Dry-run — no file changes ──
  println!("\n--- Dry-run bump ---");
  apply_release_plan(&workspace, &plan, &config, &oxrls_dir, true, false).unwrap();
  assert_version(&tmp.path().join("packages/core"), "1.0.0");
  assert_version(&tmp.path().join("packages/ui"), "2.0.0");
  assert_version(&tmp.path().join("packages/icons"), "2.0.0");
  assert_version(&tmp.path().join("packages/app"), "3.0.0");
  assert_version(&tmp.path().join("packages/web"), "3.0.0");
  assert_version(&tmp.path().join("packages/internal"), "1.0.0");
  assert_version(&tmp.path().join("packages/tools"), "1.5.0");
  assert_version(&tmp.path().join("packages/utils"), "1.0.0");
  assert_eq!(find_release_files(&oxrls_dir).unwrap().len(), 6);

  // ── Phase 6: Real bump ──
  println!("\n--- Real bump ---");
  apply_release_plan(&workspace, &plan, &config, &oxrls_dir, false, false).unwrap();

  assert_version(&tmp.path().join("packages/core"), "1.0.1");
  assert_version(&tmp.path().join("packages/ui"), "2.1.0");
  assert_version(&tmp.path().join("packages/icons"), "2.1.0");
  assert_version(&tmp.path().join("packages/app"), "4.0.0");
  assert_version(&tmp.path().join("packages/web"), "4.0.0");
  assert_version(&tmp.path().join("packages/internal"), "1.1.0-alpha.1");
  assert_version(&tmp.path().join("packages/tools"), "1.5.1-beta.1");
  assert_version(&tmp.path().join("packages/utils"), "1.0.0");
  assert_eq!(
    find_release_files(&oxrls_dir).unwrap().len(),
    0,
    "Release files consumed"
  );

  // ── Phase 7: Internal dependency updates ──
  let utils_pkg = PackageJson::read(&tmp.path().join("packages/utils/package.json")).unwrap();
  let deps = utils_pkg.dependencies.unwrap();
  assert_eq!(deps.get("@scope/core").map(|s| s.as_str()), Some("^1.0.1"));

  // ── Phase 8: Pre-mode counter bump ──
  println!("\n--- Second pre-release bump (counter) ---");
  let mut r6 = IndexMap::new();
  r6.insert("@scope/internal".to_string(), BumpType::Patch);
  create_release_file(&oxrls_dir, &r6, "Second internal fix", None).unwrap();

  let workspace2 = load_workspace(tmp.path()).unwrap();
  let plan2 = build_release_plan(&workspace2, &config, &oxrls_dir).unwrap();

  assert_eq!(plan2.bumps.len(), 1);
  let internal2 = plan2.bumps.get("@scope/internal").unwrap();
  assert_eq!(
    internal2.old_version,
    Version::parse("1.1.0-alpha.1").unwrap()
  );
  // Subsequent pre-release: base stays 1.1.0, counter → 2
  assert_eq!(
    internal2.new_version,
    Version::parse("1.1.0-alpha.2").unwrap()
  );

  apply_release_plan(&workspace2, &plan2, &config, &oxrls_dir, false, false).unwrap();
  assert_version(&tmp.path().join("packages/internal"), "1.1.0-alpha.2");

  let pre_state = oxrls::prerelease::PreState::load(&oxrls_dir).unwrap();
  assert_eq!(
    pre_state.pre_versions.get("@scope/internal").unwrap().count,
    2
  );

  println!("\n✅ Megatest passed: all 8 phases verified successfully!");
}
