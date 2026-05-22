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
    if let Some(changes) = entry.changes.get(bump_type)
      && !changes.is_empty() {
        lines.push(format!("### {}", heading));
        lines.push(String::new());
        for change in changes {
          // Indent continuation lines by 2 spaces so they stay in the list item
          let indented: String = change
            .lines()
            .enumerate()
            .map(|(i, line)| {
              if i == 0 {
                format!("- {}", line)
              } else if line.trim().is_empty() {
                String::new()
              } else {
                format!("  {}", line)
              }
            })
            .collect::<Vec<_>>()
            .join("\n");
          lines.push(indented);
        }
        lines.push(String::new());
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
  new_section: &str,
) -> Result<()> {
  // Always use # Changelog as the top-level title
  let content = if changelog_path.exists() {
    let existing = std::fs::read_to_string(changelog_path)
      .map_err(|e| OxrlsError::Changelog(format!("Failed to read changelog: {}", e)))?;
    format!("{}\n\n{}", new_section, existing)
  } else {
    format!("# Changelog\n\n{}", new_section)
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

/// Generate a global changelog section that aggregates changes across all bumped packages.
///
/// Each entry is prefixed with the package name. Changes are grouped by bump type.
/// Uses the current date as the version heading.
pub fn generate_global_changelog_section(
  packages: &[(String, semver::Version, BumpType, Vec<String>)],
) -> String {
  use chrono::Utc;

  let mut lines = Vec::new();

  let date_str = Utc::now().format("%Y-%m-%d").to_string();
  lines.push(format!("## {}", date_str));
  lines.push(String::new());

  // Group entries by bump type
  let mut major_entries: Vec<String> = Vec::new();
  let mut minor_entries: Vec<String> = Vec::new();
  let mut patch_entries: Vec<String> = Vec::new();

  for (pkg_name, version, bump_type, summaries) in packages {
    for summary in summaries {
      // Indent continuation lines by 2 spaces
      let entry: String = summary
        .lines()
        .enumerate()
        .map(|(i, line)| {
          if i == 0 {
            format!("- **{}** (v{}): {}", pkg_name, version, line)
          } else if line.trim().is_empty() {
            String::new()
          } else {
            format!("  {}", line)
          }
        })
        .collect::<Vec<_>>()
        .join("\n");
      match bump_type {
        BumpType::Major => major_entries.push(entry),
        BumpType::Minor => minor_entries.push(entry),
        BumpType::Patch => patch_entries.push(entry),
      }
    }
  }

  let sections: Vec<(&str, &Vec<String>)> = vec![
    ("Major Changes", &major_entries),
    ("Minor Changes", &minor_entries),
    ("Patch Changes", &patch_entries),
  ];

  for (heading, entries) in sections {
    if !entries.is_empty() {
      lines.push(format!("### {}", heading));
      lines.push(String::new());
      for entry in entries {
        lines.push(entry.clone());
      }
      lines.push(String::new());
    }
  }

  // Remove trailing newline
  while lines.last().map(|s| s.is_empty()).unwrap_or(false) {
    lines.pop();
  }

  if lines.is_empty() {
    return String::new();
  }

  lines.join("\n")
}

/// Update a global CHANGELOG.md in the project root.
pub fn update_global_changelog(
  changelog_path: &Path,
  new_section: &str,
) -> Result<()> {
  if new_section.is_empty() {
    return Ok(());
  }

  let content = if changelog_path.exists() {
    let existing = std::fs::read_to_string(changelog_path)
      .map_err(|e| OxrlsError::Changelog(format!("Failed to read changelog: {}", e)))?;
    format!("{}\n\n{}", new_section, existing)
  } else {
    format!("# Changelog\n\n{}", new_section)
  };

  if let Some(parent) = changelog_path.parent() {
    std::fs::create_dir_all(parent).map_err(OxrlsError::Io)?;
  }

  std::fs::write(changelog_path, &content)
    .map_err(|e| OxrlsError::Changelog(format!("Failed to write changelog: {}", e)))?;

  Ok(())
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
