use crate::bump::plan::PlannedBump;
use crate::error::{OxrlsError, Result};
use crate::release_file::BumpType;
use std::path::{Path, PathBuf};

/// Find all markdown files in the release directory.
pub fn find_release_files(release_dir: &Path) -> Result<Vec<PathBuf>> {
  if !release_dir.exists() {
    return Ok(vec![]);
  }

  let mut files = Vec::new();
  let entries = std::fs::read_dir(release_dir).map_err(OxrlsError::Io)?;

  for entry in entries {
    let entry = entry.map_err(OxrlsError::Io)?;
    let path = entry.path();
    if path.is_file() && path.extension().map(|e| e == "md").unwrap_or(false) {
      // Skip README.md
      if path.file_stem().map(|s| s == "README").unwrap_or(false) {
        continue;
      }
      files.push(path);
    }
  }

  files.sort();
  Ok(files)
}

impl PlannedBump {
  pub fn bump_type_str(&self) -> &str {
    match self.bump_type {
      BumpType::Patch => "patch",
      BumpType::Minor => "minor",
      BumpType::Major => "major",
    }
  }
}
