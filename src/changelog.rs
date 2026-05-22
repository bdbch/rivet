use crate::error::{OxrlsError, Result};
use crate::release_file::BumpType;
use indexmap::IndexMap;
use std::path::Path;

/// Represents a single changelog entry for one version.
#[derive(Debug, Clone)]
pub struct ChangelogEntry {
  /// Package name
  pub package_name: String,
  /// New version string
  pub version: String,
  /// Changes grouped by type
  pub changes: IndexMap<BumpType, Vec<String>>,
}

/// Generate the changelog section string for a single version.
pub fn generate_changelog_section(entry: &ChangelogEntry) -> String {
  let mut lines = Vec::new();

  lines.push(format!("## {}", entry.version));
  lines.push(String::new());

  let type_names = [
    ("Major Changes", BumpType::Major),
    ("Minor Changes", BumpType::Minor),
    ("Patch Changes", BumpType::Patch),
  ];

  for (heading, bump_type) in &type_names {
    if let Some(changes) = entry.changes.get(bump_type) {
      if !changes.is_empty() {
        lines.push(format!("### {}", heading));
        lines.push(String::new());
        for change in changes {
          lines.push(format!("- {}", change));
        }
        lines.push(String::new());
      }
    }
  }

  // Remove trailing newline
  while lines.last().map(|s| s.is_empty()).unwrap_or(false) {
    lines.pop();
  }

  lines.join("\n")
}

/// Update a CHANGELOG.md file with a new version entry.
/// Creates the file if it doesn't exist, prepends the new entry if it does.
pub fn update_changelog(
  changelog_path: &Path,
  package_name: &str,
  new_section: &str,
) -> Result<()> {
  let content = if changelog_path.exists() {
    let existing = std::fs::read_to_string(changelog_path)
      .map_err(|e| OxrlsError::Changelog(format!("Failed to read changelog: {}", e)))?;
    format!("{}\n\n{}", new_section, existing)
  } else {
    format!("# {}\n\n{}", package_name, new_section)
  };

  // Ensure parent directory exists
  if let Some(parent) = changelog_path.parent() {
    std::fs::create_dir_all(parent).map_err(OxrlsError::Io)?;
  }

  std::fs::write(changelog_path, &content)
    .map_err(|e| OxrlsError::Changelog(format!("Failed to write changelog: {}", e)))?;

  Ok(())
}

/// Group release file summaries by bump type for a given package.
pub fn group_changes_by_type(
  release_summaries: &[(&str, BumpType)],
) -> IndexMap<BumpType, Vec<String>> {
  let mut changes: IndexMap<BumpType, Vec<String>> = IndexMap::new();

  for (summary, bump_type) in release_summaries {
    changes
      .entry(*bump_type)
      .or_default()
      .push(summary.to_string());
  }

  changes
}

#[cfg(test)]
mod tests {
  use super::*;
  use indexmap::IndexMap;

  #[test]
  fn test_changelog_section_patch() {
    let mut changes = IndexMap::new();
    changes.insert(
      BumpType::Patch,
      vec!["Fixed editor selection behavior.".to_string()],
    );

    let entry = ChangelogEntry {
      package_name: "@scope/core".to_string(),
      version: "1.2.4".to_string(),
      changes,
    };

    let section = generate_changelog_section(&entry);
    assert!(section.contains("1.2.4"));
    assert!(section.contains("### Patch Changes"));
    assert!(section.contains("Fixed editor selection behavior."));
    assert!(!section.contains("Minor Changes"));
  }

  #[test]
  fn test_changelog_section_multiple_types() {
    let mut changes = IndexMap::new();
    changes.insert(
      BumpType::Major,
      vec!["Breaking: removed old API.".to_string()],
    );
    changes.insert(
      BumpType::Minor,
      vec!["Added new helper functions.".to_string()],
    );
    changes.insert(BumpType::Patch, vec!["Fixed minor bug.".to_string()]);

    let entry = ChangelogEntry {
      package_name: "@scope/core".to_string(),
      version: "2.0.0".to_string(),
      changes,
    };

    let section = generate_changelog_section(&entry);
    assert!(section.contains("### Major Changes"));
    assert!(section.contains("### Minor Changes"));
    assert!(section.contains("### Patch Changes"));
    assert!(section.contains("Breaking: removed old API."));
    assert!(section.contains("Added new helper functions."));
    assert!(section.contains("Fixed minor bug."));
  }

  #[test]
  fn test_group_changes_by_type() {
    let summaries = vec![
      ("Fix bug A", BumpType::Patch),
      ("Add feature B", BumpType::Minor),
      ("Fix bug C", BumpType::Patch),
    ];

    let grouped = group_changes_by_type(&summaries);
    assert_eq!(grouped.get(&BumpType::Patch).unwrap().len(), 2);
    assert_eq!(grouped.get(&BumpType::Minor).unwrap().len(), 1);
    assert!(grouped.get(&BumpType::Major).is_none());
  }
}
