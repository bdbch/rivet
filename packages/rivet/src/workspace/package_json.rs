use crate::error::{Result, RivetError};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Represents a package.json file. Includes only fields we care about.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageJson {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub version: Option<String>,

  #[serde(default)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub private: Option<bool>,

  #[serde(default)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub dependencies: Option<IndexMap<String, String>>,

  #[serde(default)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub dev_dependencies: Option<IndexMap<String, String>>,

  #[serde(default)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub peer_dependencies: Option<IndexMap<String, String>>,

  #[serde(default)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub optional_dependencies: Option<IndexMap<String, String>>,

  /// Catch any other fields we don't care about but need to preserve.
  #[serde(flatten)]
  pub extra: BTreeMap<String, serde_json::Value>,
}

impl PackageJson {
  /// Read a package.json file, preserving unknown fields.
  pub fn read(path: &Path) -> Result<Self> {
    let content = std::fs::read_to_string(path)
      .map_err(|e| RivetError::Package(format!("Failed to read {}: {}", path.display(), e)))?;
    let pkg: PackageJson = serde_json::from_str(&content)?;
    Ok(pkg)
  }

  /// Write a package.json file with 2-space indentation.
  pub fn write(path: &Path, pkg: &Self) -> Result<()> {
    let json_str = serde_json::to_string_pretty(pkg)?;
    let with_newline = json_str + "\n";
    std::fs::write(path, with_newline)
      .map_err(|e| RivetError::Package(format!("Failed to write {}: {}", path.display(), e)))?;
    Ok(())
  }

  /// Get the version as a semver Version.
  pub fn semver_version(&self) -> Result<semver::Version> {
    let v = self
      .version
      .as_deref()
      .ok_or_else(|| RivetError::Package("Missing version field".to_string()))?;
    semver::Version::parse(v)
      .map_err(|e| RivetError::Version(format!("Invalid version '{}': {}", v, e)))
  }

  /// Set the version from a semver Version.
  pub fn set_version(&mut self, version: &semver::Version) {
    self.version = Some(version.to_string());
  }
}

/// Given a current dependency range like "^1.2.3", "~0.4.0", "workspace:^1.0.0", etc.,
/// compute the updated range after a version bump.
pub fn compute_new_range(
  range: &str,
  old_version: &semver::Version,
  new_version: &semver::Version,
) -> String {
  // Handle workspace: protocol
  if let Some(inner) = range.strip_prefix("workspace:") {
    match inner {
      "*" | "^" | "~" => {
        // Keep these as-is
        return range.to_string();
      }
      _ => {
        // Try to parse as a version range
        let new_inner = compute_simple_range(inner, old_version, new_version);
        return format!("workspace:{}", new_inner);
      }
    }
  }

  compute_simple_range(range, old_version, new_version)
}

/// Compute the new range for a non-workspace range.
fn compute_simple_range(
  range: &str,
  old_version: &semver::Version,
  new_version: &semver::Version,
) -> String {
  let trimmed = range.trim();

  if trimmed == "*" || trimmed == "x" || trimmed == "X" {
    return range.to_string();
  }

  // Get the range prefix character
  let prefix = get_range_prefix(trimmed);
  let prefix_len = prefix.len();

  // Try to extract the version part after the prefix
  let rest = &trimmed[prefix_len..].trim();

  // Try to parse the rest as a semver version
  if let Ok(ver) = semver::Version::parse(rest) {
    // Only update if the version in the range corresponds to the old_version:
    //
    //   Prefix  |  Condition for "matches this dependency"
    //   --------+------------------------------------------
    //   ^       |  Major version matches (caret allows any minor/patch)
    //   ~       |  Major AND minor match (tilde allows only patch)
    //   (none)  |  Major AND minor match (exact match on patch too, but
    //          |   already verified by parsing rest == old_version's string)
    //   >= etc. |  Major AND minor match (conservative: keep in sync)
    let version_matches = match prefix {
      "^" => ver.major == old_version.major,
      "" | "~" => ver.major == old_version.major && ver.minor == old_version.minor,
      _ => ver.major == old_version.major && ver.minor == old_version.minor,
    };

    if version_matches {
      // Construct the new version with the same prefix
      let new_ver_str = format_version_like(rest, new_version);
      return format!("{}{}", prefix, new_ver_str);
    }
  }

  // Fallback: replace the old version string within the range,
  // but only when bounded by non-digit characters or string edges
  // to avoid false positives like replacing "1.2.3" inside "^1.2.30".
  let old_str = old_version.to_string();
  if let Some(pos) = trimmed.find(&old_str) {
    let before = &trimmed[..pos];
    let after = &trimmed[pos + old_str.len()..];
    let at_start = pos == 0;
    let at_end = pos + old_str.len() == trimmed.len();
    let prev_is_non_digit = pos > 0 && !trimmed.as_bytes()[pos - 1].is_ascii_digit();
    let next_is_non_digit = pos + old_str.len() < trimmed.len()
      && !trimmed.as_bytes()[pos + old_str.len()].is_ascii_digit();
    if (at_start || prev_is_non_digit) && (at_end || next_is_non_digit) {
      let new_str = new_version.to_string();
      return format!("{}{}{}", before, new_str, after);
    }
  }

  range.to_string()
}

/// Extract the range prefix (^, ~, >=, <=, >, <, or empty).
fn get_range_prefix(range: &str) -> &str {
  let trimmed = range.trim();
  if trimmed.starts_with(">=") {
    ">="
  } else if trimmed.starts_with("<=") {
    "<="
  } else if trimmed.starts_with('^') {
    "^"
  } else if trimmed.starts_with('~') {
    "~"
  } else if trimmed.starts_with('>') {
    ">"
  } else if trimmed.starts_with('<') {
    "<"
  } else {
    ""
  }
}

/// Format a new version string to match the precision of the old version string.
fn format_version_like(old_version_str: &str, new_version: &semver::Version) -> String {
  let parts: Vec<&str> = old_version_str.split('.').collect();
  match parts.len() {
    1 => format!("{}", new_version.major),
    2 => format!("{}.{}", new_version.major, new_version.minor),
    _ => format!(
      "{}.{}.{}",
      new_version.major, new_version.minor, new_version.patch
    ),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_update_caret_range() {
    let old = semver::Version::new(1, 2, 3);
    let new = semver::Version::new(1, 2, 4);
    let result = compute_new_range("^1.2.3", &old, &new);
    assert_eq!(result, "^1.2.4");
  }

  #[test]
  fn test_update_tilde_range() {
    let old = semver::Version::new(1, 2, 3);
    let new = semver::Version::new(1, 2, 4);
    let result = compute_new_range("~1.2.3", &old, &new);
    assert_eq!(result, "~1.2.4");
  }

  #[test]
  fn test_update_exact_range() {
    let old = semver::Version::new(1, 2, 3);
    let new = semver::Version::new(1, 2, 4);
    let result = compute_new_range("1.2.3", &old, &new);
    assert_eq!(result, "1.2.4");
  }

  #[test]
  fn test_workspace_star_unchanged() {
    let old = semver::Version::new(1, 2, 3);
    let new = semver::Version::new(2, 0, 0);
    let result = compute_new_range("workspace:*", &old, &new);
    assert_eq!(result, "workspace:*");
  }

  #[test]
  fn test_workspace_caret() {
    let old = semver::Version::new(1, 2, 3);
    let new = semver::Version::new(1, 2, 4);
    let result = compute_new_range("workspace:^1.2.3", &old, &new);
    assert_eq!(result, "workspace:^1.2.4");
  }

  #[test]
  fn test_workspace_tilde_unchanged() {
    let old = semver::Version::new(1, 2, 3);
    let new = semver::Version::new(1, 2, 4);
    let result = compute_new_range("workspace:~", &old, &new);
    assert_eq!(result, "workspace:~");
  }

  #[test]
  fn test_workspace_exact() {
    let old = semver::Version::new(1, 2, 3);
    let new = semver::Version::new(1, 2, 4);
    let result = compute_new_range("workspace:1.2.3", &old, &new);
    assert_eq!(result, "workspace:1.2.4");
  }

  #[test]
  fn test_wildcard_unchanged() {
    let old = semver::Version::new(1, 2, 3);
    let new = semver::Version::new(2, 0, 0);
    let result = compute_new_range("*", &old, &new);
    assert_eq!(result, "*");
  }
}
