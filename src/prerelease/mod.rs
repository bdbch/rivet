pub mod logic;
pub mod state;

pub use logic::*;
pub use state::*;

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::{OxrlsConfig, PreModeEntry};

  #[test]
  fn test_pre_state_increment() {
    let mut state = PreState::default();
    assert_eq!(state.get_count("@scope/core", "beta"), 0);

    let c1 = state.increment("@scope/core", "beta");
    assert_eq!(c1, 1);
    assert_eq!(state.get_count("@scope/core", "beta"), 1);

    let c2 = state.increment("@scope/core", "beta");
    assert_eq!(c2, 2);
  }

  #[test]
  fn test_pre_state_replacing_tag_resets_count() {
    let mut state = PreState::default();
    state.increment("@scope/pkg", "beta");
    state.increment("@scope/pkg", "beta");

    assert_eq!(state.get_count("@scope/pkg", "beta"), 2);

    // Moving to a new tag replaces the entry, resetting the counter
    state.increment("@scope/pkg", "alpha");
    assert_eq!(state.get_count("@scope/pkg", "alpha"), 1);
    assert_eq!(state.get_count("@scope/pkg", "beta"), 0);
  }

  #[test]
  fn test_resolve_pre_release() {
    let config = OxrlsConfig {
      pre_mode: vec![PreModeEntry {
        tag: "beta".to_string(),
        packages: vec!["@scope/core".to_string(), "@scope/react".to_string()],
      }],
      ..Default::default()
    };
    let mut pre_state = PreState::default();

    let result = resolve_pre_release("@scope/core", &config, &mut pre_state);
    assert_eq!(result, Some(("beta".to_string(), 1)));

    // Second call increments
    let result = resolve_pre_release("@scope/core", &config, &mut pre_state);
    assert_eq!(result, Some(("beta".to_string(), 2)));

    // Package not in pre-mode
    let result = resolve_pre_release("@scope/other", &config, &mut pre_state);
    assert_eq!(result, None);
  }

  #[test]
  fn test_resolve_with_glob() {
    let config = OxrlsConfig {
      pre_mode: vec![PreModeEntry {
        tag: "alpha".to_string(),
        packages: vec!["@scope/pre-*".to_string()],
      }],
      ..Default::default()
    };
    let mut pre_state = PreState::default();

    let result = resolve_pre_release("@scope/pre-alpha", &config, &mut pre_state);
    assert_eq!(result, Some(("alpha".to_string(), 1)));

    // Should not match
    let result = resolve_pre_release("@scope/other", &config, &mut pre_state);
    assert_eq!(result, None);
  }

  #[test]
  fn test_apply_pre_release() {
    let base = semver::Version::new(2, 0, 0);
    let result = apply_pre_release(&base, "beta", 1).unwrap();
    assert_eq!(result.to_string(), "2.0.0-beta.1");

    let result = apply_pre_release(&base, "beta", 3).unwrap();
    assert_eq!(result.to_string(), "2.0.0-beta.3");

    let result = apply_pre_release(&base, "rc", 1).unwrap();
    assert_eq!(result.to_string(), "2.0.0-rc.1");
  }

  #[test]
  fn test_pre_state_persistence() {
    let tmp = tempfile::TempDir::new().unwrap();
    let release_dir = tmp.path().join(".oxrls");

    let mut state = PreState::default();
    state.increment("@scope/core", "beta");
    state.increment("@scope/core", "beta");
    state.save(&release_dir).unwrap();

    let loaded = PreState::load(&release_dir).unwrap();
    assert_eq!(loaded.get_count("@scope/core", "beta"), 2);
  }

  #[test]
  fn test_pre_state_remove() {
    let mut state = PreState::default();
    state.increment("@scope/pkg", "beta");
    assert!(state.is_in_pre("@scope/pkg"));

    state.remove("@scope/pkg");
    assert!(!state.is_in_pre("@scope/pkg"));
  }

  #[test]
  fn test_apply_pre_release_invalid_tag_returns_error() {
    let base = semver::Version::new(1, 0, 0);

    // Empty tag should fail (empty pre-release identifier not allowed by semver)
    let result = apply_pre_release(&base, "", 1);
    assert!(result.is_err(), "Empty tag should produce an error");

    // Tag with special characters should fail
    // (semver prerelease only allows alphanumeric and hyphens)
    let result = apply_pre_release(&base, "beta!@#", 1);
    assert!(
      result.is_err(),
      "Tag with special chars should produce an error"
    );

    // Valid tags should still work
    let result = apply_pre_release(&base, "beta", 1);
    assert!(result.is_ok(), "Valid tag should succeed");
    assert_eq!(result.unwrap().to_string(), "1.0.0-beta.1");
  }
}
