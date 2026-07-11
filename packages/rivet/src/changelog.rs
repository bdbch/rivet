use crate::error::{Result, RivetError};
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

/// Format a multi-line change entry into a markdown list item.
/// The first line is prefixed with `first_line_prefix`, continuation lines
/// are indented by 2 spaces so they stay inside the list item.
fn indent_continuations(text: &str, first_line_prefix: &str) -> String {
  text
    .lines()
    .enumerate()
    .map(|(i, line)| {
      if i == 0 {
        format!("{}{}", first_line_prefix, line)
      } else if line.trim().is_empty() {
        String::new()
      } else {
        format!("  {}", line)
      }
    })
    .collect::<Vec<_>>()
    .join("\n")
}

/// Generate the changelog section string for a single version.
pub fn generate_changelog_section(entry: &ChangelogEntry) -> String {
  let mut lines = Vec::new();

  lines.push(format!("## v{}", entry.version));
  lines.push(String::new());

  let type_names = [
    ("Major Changes", BumpType::Major),
    ("Minor Changes", BumpType::Minor),
    ("Patch Changes", BumpType::Patch),
  ];

  for (heading, bump_type) in &type_names {
    if let Some(changes) = entry.changes.get(bump_type)
      && !changes.is_empty()
    {
      lines.push(format!("### {}", heading));
      lines.push(String::new());
      for change in changes {
        lines.push(indent_continuations(change, "- "));
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

/// Strip the `# Changelog` header from existing changelog content.
/// Handles standard headers, missing trailing newlines, and case variations.
fn strip_changelog_header(content: &str) -> &str {
  content
    .strip_prefix("# Changelog\n\n")
    .or_else(|| content.strip_prefix("# Changelog\n"))
    .or_else(|| {
      // Fallback: find any heading that starts with "# Changelog"
      let trimmed = content.trim_start();
      if trimmed.to_lowercase().starts_with("# changelog") {
        let end = trimmed.find('\n').map(|i| i + 1).unwrap_or(0);
        Some(&trimmed[end..])
      } else {
        None
      }
    })
    .unwrap_or(content)
}

/// Update a CHANGELOG.md file with a new version entry.
/// Creates the file if it doesn't exist, prepends the new entry if it does.
pub fn update_changelog(changelog_path: &Path, new_section: &str) -> Result<()> {
  // Always use # Changelog as the top-level title
  let content = if changelog_path.exists() {
    let existing = std::fs::read_to_string(changelog_path)
      .map_err(|e| RivetError::Changelog(format!("Failed to read changelog: {}", e)))?;
    let body = strip_changelog_header(&existing);
    format!("# Changelog\n\n{}\n\n{}", new_section, body.trim())
  } else {
    format!("# Changelog\n\n{}", new_section)
  };

  // Ensure parent directory exists
  if let Some(parent) = changelog_path.parent() {
    std::fs::create_dir_all(parent).map_err(RivetError::Io)?;
  }

  std::fs::write(changelog_path, &content)
    .map_err(|e| RivetError::Changelog(format!("Failed to write changelog: {}", e)))?;

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
/// Each bumped package produces its own `## {package_name} (v{version})` heading
/// followed by per-bump-type subsections (`### Patch Changes`, etc.) with plain bullets.
/// No date grouping is used — the package+version pair is unique per release, which is
/// the natural unit of grouping in a monorepo.
pub fn generate_global_changelog_section(entries: &[ChangelogEntry]) -> String {
  let type_names = [
    ("Major Changes", BumpType::Major),
    ("Minor Changes", BumpType::Minor),
    ("Patch Changes", BumpType::Patch),
  ];

  let mut sections: Vec<String> = Vec::new();

  for entry in entries {
    let mut entry_lines = Vec::new();
    entry_lines.push(format!("## {} (v{})", entry.package_name, entry.version));
    entry_lines.push(String::new());

    let mut any_changes = false;

    for (heading, bump_type) in &type_names {
      if let Some(changes) = entry.changes.get(bump_type)
        && !changes.is_empty()
      {
        entry_lines.push(format!("### {}", heading));
        entry_lines.push(String::new());
        for change in changes {
          entry_lines.push(indent_continuations(change, "- "));
        }
        entry_lines.push(String::new());
        any_changes = true;
      }
    }

    if !any_changes {
      // Skip packages with no recorded summaries so we never emit a bare heading
      continue;
    }

    while entry_lines.last().map(|s| s.is_empty()).unwrap_or(false) {
      entry_lines.pop();
    }

    sections.push(entry_lines.join("\n"));
  }

  if sections.is_empty() {
    return String::new();
  }

  sections.join("\n\n")
}

/// Update a global CHANGELOG.md in the project root.
pub fn update_global_changelog(changelog_path: &Path, new_section: &str) -> Result<()> {
  if new_section.is_empty() {
    return Ok(());
  }

  let content = if changelog_path.exists() {
    let existing = std::fs::read_to_string(changelog_path)
      .map_err(|e| RivetError::Changelog(format!("Failed to read changelog: {}", e)))?;
    let body = strip_changelog_header(&existing);
    format!("# Changelog\n\n{}\n\n{}", new_section, body.trim())
  } else {
    format!("# Changelog\n\n{}", new_section)
  };

  if let Some(parent) = changelog_path.parent() {
    std::fs::create_dir_all(parent).map_err(RivetError::Io)?;
  }

  std::fs::write(changelog_path, &content)
    .map_err(|e| RivetError::Changelog(format!("Failed to write changelog: {}", e)))?;

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
    assert!(section.contains("v1.2.4"));
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
    assert!(section.contains("v2.0.0"));
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

  #[test]
  fn test_strip_changelog_header_standard() {
    let content = "# Changelog\n\n## 1.0.0\n\n### Patch Changes\n\n- Fix bug\n";
    let body = strip_changelog_header(content);
    assert_eq!(body, "## 1.0.0\n\n### Patch Changes\n\n- Fix bug\n");
  }

  #[test]
  fn test_strip_changelog_header_no_trailing_newline() {
    let content = "# Changelog\n## 1.0.0\n\n- Fix bug\n";
    let body = strip_changelog_header(content);
    assert_eq!(body, "## 1.0.0\n\n- Fix bug\n");
  }

  #[test]
  fn test_strip_changelog_header_case_insensitive() {
    let content = "# CHANGELOG\n\n## 1.0.0\n\n- Fix bug\n";
    let body = strip_changelog_header(content);
    // The fallback strips past the first \n (right after "CHANGELOG"),
    // leaving the blank line which the caller's .trim() removes.
    assert_eq!(body, "\n## 1.0.0\n\n- Fix bug\n");
  }

  #[test]
  fn test_strip_changelog_header_no_header() {
    let content = "## 1.0.0\n\n- Fix bug\n";
    let body = strip_changelog_header(content);
    // No header to strip — returns original
    assert_eq!(body, content);
  }

  #[test]
  fn test_indent_continuations_single_line() {
    let result = indent_continuations("Fix bug", "- ");
    assert_eq!(result, "- Fix bug");
  }

  #[test]
  fn test_indent_continuations_multi_line() {
    let result = indent_continuations("Fix bug\nWith details\nMore info", "- ");
    assert_eq!(result, "- Fix bug\n  With details\n  More info");
  }

  #[test]
  fn test_indent_continuations_empty_lines_skipped() {
    let result = indent_continuations("Header\n\n\nTrailing", "- ");
    assert_eq!(result, "- Header\n\n\n  Trailing");
  }

  #[test]
  fn test_indent_continuations_custom_prefix() {
    let result = indent_continuations("Fix bug\nDetails", "- **pkg** (v1.0.0): ");
    assert_eq!(result, "- **pkg** (v1.0.0): Fix bug\n  Details");
  }

  fn make_global_entry(
    name: &str,
    version: &str,
    changes: Vec<(&str, BumpType)>,
  ) -> ChangelogEntry {
    let mut grouped: IndexMap<BumpType, Vec<String>> = IndexMap::new();
    for (summary, bt) in changes {
      grouped.entry(bt).or_default().push(summary.to_string());
    }
    ChangelogEntry {
      package_name: name.to_string(),
      version: version.to_string(),
      changes: grouped,
    }
  }

  #[test]
  fn test_global_changelog_section_single_patch() {
    let entry = make_global_entry(
      "@bdbchgg/rivet",
      "1.0.0-alpha.5",
      vec![(
        "Fixed a bug that caused the binary not to be included with the bundle",
        BumpType::Patch,
      )],
    );

    let section = generate_global_changelog_section(&[entry]);
    assert_eq!(
      section,
      "## @bdbchgg/rivet (v1.0.0-alpha.5)\n\n\
        ### Patch Changes\n\n\
        - Fixed a bug that caused the binary not to be included with the bundle"
    );
  }

  #[test]
  fn test_global_changelog_section_no_package_prefix_in_bullets() {
    let entry = make_global_entry(
      "@scope/core",
      "2.1.0",
      vec![
        ("Add feature", BumpType::Minor),
        ("Fix bug", BumpType::Patch),
      ],
    );

    let section = generate_global_changelog_section(&[entry]);
    assert!(!section.contains("- **@scope/core**"));
    assert!(section.contains("- Add feature"));
    assert!(section.contains("- Fix bug"));
  }

  #[test]
  fn test_global_changelog_section_multi_package() {
    let pkg_a = make_global_entry(
      "@bdbchgg/rivet",
      "1.0.0-alpha.5",
      vec![("Fixed bug A", BumpType::Patch)],
    );
    let pkg_b = make_global_entry(
      "@bdbchgg/other",
      "0.4.0",
      vec![("Add helper", BumpType::Minor)],
    );

    let section = generate_global_changelog_section(&[pkg_a, pkg_b]);
    assert!(
      section.contains("## @bdbchgg/rivet (v1.0.0-alpha.5)\n\n### Patch Changes\n\n- Fixed bug A"),
      "Expected first package section, got:\n{}",
      section,
    );
    assert!(
      section.contains("## @bdbchgg/other (v0.4.0)\n\n### Minor Changes\n\n- Add helper"),
      "Expected second package section, got:\n{}",
      section,
    );
  }

  #[test]
  fn test_global_changelog_section_multiple_bump_types_one_package() {
    let entry = make_global_entry(
      "@scope/core",
      "3.0.0",
      vec![
        ("Breaking: removed old API", BumpType::Major),
        ("Added helper", BumpType::Minor),
        ("Fixed bug", BumpType::Patch),
      ],
    );

    let section = generate_global_changelog_section(&[entry]);
    let headings = [
      "### Major Changes",
      "### Minor Changes",
      "### Patch Changes",
    ];
    for heading in headings {
      assert!(
        section.contains(heading),
        "Expected section heading '{}' in:\n{}",
        heading,
        section,
      );
    }
    assert_eq!(section.lines().filter(|l| l.starts_with("## ")).count(), 1);
  }

  #[test]
  fn test_global_changelog_section_skips_entry_without_changes() {
    let empty_entry = ChangelogEntry {
      package_name: "@scope/untouched".to_string(),
      version: "1.0.0".to_string(),
      changes: IndexMap::new(),
    };
    let real_entry = make_global_entry("@scope/core", "1.1.0", vec![("Hello", BumpType::Minor)]);

    let section = generate_global_changelog_section(&[empty_entry, real_entry]);
    assert!(!section.contains("@scope/untouched"));
    assert!(section.contains("@scope/core"));
  }
}
