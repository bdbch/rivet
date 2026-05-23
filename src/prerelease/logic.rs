use crate::config::OxrlsConfig;
use crate::error::Result;
use crate::prerelease::state::PreState;
use glob::Pattern;

/// Determine the pre-release tag and counter for a package, if it's in pre-mode.
/// Returns `Some((tag, counter))` if the package should produce a pre-release version.
/// The counter is the *next* value to use (already incremented in the returned state).
pub fn resolve_pre_release(
  package_name: &str,
  config: &OxrlsConfig,
  pre_state: &mut PreState,
) -> Option<(String, u64)> {
  // Check the package against each preMode entry
  for entry in &config.pre_mode {
    let matches = entry.packages.iter().any(|pattern| {
      if let Ok(pat) = Pattern::new(pattern) {
        pat.matches(package_name)
      } else {
        eprintln!(
          "Warning: invalid glob pattern \"{}\" in pre-mode config, falling back to exact match",
          pattern
        );
        package_name == pattern
      }
    });

    if matches {
      // Increment and get the new counter value
      let count = pre_state.increment(package_name, &entry.tag);
      return Some((entry.tag.clone(), count));
    }
  }

  None
}

/// Apply a pre-release tag and counter to a base version string.
pub fn apply_pre_release(
  base_version: &semver::Version,
  tag: &str,
  count: u64,
) -> Result<semver::Version> {
  let pre = format!("{}.{}", tag, count);
  let prerelease = semver::Prerelease::new(&pre).map_err(|e| {
    crate::error::OxrlsError::Version(format!(
      "Invalid pre-release identifier '{}': {}",
      pre, e
    ))
  })?;
  Ok(semver::Version {
    major: base_version.major,
    minor: base_version.minor,
    patch: base_version.patch,
    pre: prerelease,
    build: Default::default(),
  })
}
