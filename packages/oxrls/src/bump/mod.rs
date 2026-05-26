mod apply;
mod deps;
mod discovery;
mod groups;
mod plan;

// Public API — re-export everything that was `pub` in the original bump.rs
pub use apply::{apply_release_plan, print_plan};
pub use discovery::find_release_files;
pub use plan::{InternalDepUpdateInfo, PlannedBump, ReleasePlan, build_release_plan};

// Internal items — re-exported as pub(crate) for cross-module access and tests
#[cfg(test)]
pub(crate) use groups::resolve_group_patterns;

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::OxrlsConfig;
  use crate::release_file::BumpType;
  use crate::workspace::PackageJson;
  use crate::workspace::Workspace;
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
    let patterns = vec!["@scope/*".to_string(), "!@scope/core".to_string()];
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

    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

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
      fixed: vec![vec!["@scope/*".to_string(), "!@scope/react".to_string()]],
      ..Default::default()
    };

    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

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
    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

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
    let result = build_release_plan(&workspace, &config, &release_dir);
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
    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

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
    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

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

    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

    let core = plan.bumps.get("@scope/core").unwrap();
    // Should be 1.2.4-beta.1 instead of 1.2.4
    assert_eq!(core.new_version.to_string(), "1.2.4-beta.1");

    // Persist the pre-state as `apply_release_plan` would do.
    // Pre-release counters are only saved after a successful apply,
    // so we simulate that here before the second plan build.
    plan.pre_state.save(&release_dir).unwrap();

    // Second bump should increment the counter
    let content2 = r#"---
"@scope/core": patch
---

Fix another bug."#;
    std::fs::write(release_dir.join("test2.md"), content2).unwrap();

    let plan2 = build_release_plan(&workspace, &config, &release_dir).unwrap();
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

    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

    let core = plan.bumps.get("@scope/core").unwrap();
    // Major bump from 1.2.3 -> 2.0.0-rc.1
    assert_eq!(core.new_version.to_string(), "2.0.0-rc.1");
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

    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

    let core = plan.bumps.get("@scope/core").unwrap();
    assert_eq!(core.new_version.to_string(), "1.2.4-beta.1");

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

    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

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

    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

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

    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

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
    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

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

    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

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

    let plan = build_release_plan(&workspace, &config, &release_dir).unwrap();

    // Verify both packages are in the plan
    assert!(plan.bumps.contains_key("@scope/core"));
    assert!(plan.bumps.contains_key("@scope/react"));

    // Core is in pre-mode -> should have a pre-release version
    assert!(
      plan
        .bumps
        .get("@scope/core")
        .unwrap()
        .new_version
        .to_string()
        .contains("beta")
    );
    // React is not in pre-mode -> should have a normal version
    assert!(
      !plan
        .bumps
        .get("@scope/react")
        .unwrap()
        .new_version
        .to_string()
        .contains("beta")
    );

    // Apply the release plan
    apply_release_plan(&workspace, &plan, &config, &release_dir, false, false).unwrap();

    // The release file should be consumed — changelog already captured the content
    assert!(
      !rf_path.exists(),
      "Release file should be consumed as pre-release entries were already captured"
    );
  }

  #[test]
  fn test_pre_state_not_saved_by_build_plan_alone() {
    // When build_release_plan is called without a subsequent apply,
    // the pre-release counters should NOT be persisted to disk.
    // This ensures that if apply_release_plan fails, a retry
    // produces the same pre-release version (counter stays unchanged
    // until the apply actually succeeds).
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

    // First build: counter goes 0→1 in memory only
    let plan1 = build_release_plan(&workspace, &config, &release_dir).unwrap();
    let core1 = plan1.bumps.get("@scope/core").unwrap();
    assert_eq!(core1.new_version.to_string(), "1.2.4-beta.1");

    // Second build WITHOUT saving pre-state:
    // if counters were persisted, we'd get beta.2.
    // With the atomicity fix, we should still get beta.1
    // because the save only happens in apply_release_plan.
    let plan2 = build_release_plan(&workspace, &config, &release_dir).unwrap();
    let core2 = plan2.bumps.get("@scope/core").unwrap();
    assert_eq!(core2.new_version.to_string(), "1.2.4-beta.1");

    // Now simulate the apply: save the pre-state and verify
    // the NEXT build sees the incremented counter.
    plan2.pre_state.save(&release_dir).unwrap();
    let plan3 = build_release_plan(&workspace, &config, &release_dir).unwrap();
    let core3 = plan3.bumps.get("@scope/core").unwrap();
    assert_eq!(core3.new_version.to_string(), "1.2.3-beta.2");
  }
}
