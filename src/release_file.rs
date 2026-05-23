use crate::error::{OxrlsError, Result};
use indexmap::IndexMap;
use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// The type of version bump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BumpType {
  Patch,
  Minor,
  Major,
}

impl fmt::Display for BumpType {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      BumpType::Patch => write!(f, "patch"),
      BumpType::Minor => write!(f, "minor"),
      BumpType::Major => write!(f, "major"),
    }
  }
}

impl std::str::FromStr for BumpType {
  type Err = OxrlsError;

  fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
    match s.trim().to_lowercase().as_str() {
      "patch" => Ok(BumpType::Patch),
      "minor" => Ok(BumpType::Minor),
      "major" => Ok(BumpType::Major),
      other => Err(OxrlsError::ReleaseFile(format!(
        "Unknown bump type \"{}\". Expected one of: patch, minor, major.",
        other
      ))),
    }
  }
}

impl BumpType {
  /// Priority: major > minor > patch
  pub fn priority(&self) -> u8 {
    match self {
      BumpType::Patch => 1,
      BumpType::Minor => 2,
      BumpType::Major => 3,
    }
  }

  /// Select the higher-priority bump type.
  pub fn max(a: Option<Self>, b: Self) -> Self {
    match a {
      None => b,
      Some(existing) => {
        if b.priority() > existing.priority() {
          b
        } else {
          existing
        }
      }
    }
  }
}

/// A parsed, validated release file.
#[derive(Debug, Clone)]
pub struct ReleaseFile {
  /// Path to the release file.
  pub path: PathBuf,
  /// The packages and their bump types.
  pub releases: IndexMap<String, BumpType>,
  /// The summary/body text.
  pub summary: String,
}

/// Parse a release markdown file at the given path.
pub fn parse_release_file(path: &Path) -> Result<ReleaseFile> {
  let content = fs::read_to_string(path)
    .map_err(|e| OxrlsError::ReleaseFile(format!("Failed to read {}: {}", path.display(), e)))?;

  parse_release_content(&content, path)
}

/// Parse release content from a string.
pub fn parse_release_content(content: &str, source: &Path) -> Result<ReleaseFile> {
  let trimmed = content.trim();

  if trimmed.is_empty() {
    return Err(OxrlsError::ReleaseFile(format!(
      "Release file {} is empty.",
      source.display()
    )));
  }

  // Parse frontmatter: first line must start with "---"
  // Everything between the first "---" and the next "---" is YAML.
  let (yaml_part, summary) = parse_frontmatter(trimmed).ok_or_else(|| {
    OxrlsError::ReleaseFile(format!(
      "Invalid release file {}. Missing or invalid frontmatter.\n\
             Expected format:\n---\n\"package-name\": patch\n---\n\nSummary here.",
      source.display()
    ))
  })?;

  if summary.trim().is_empty() {
    return Err(OxrlsError::ReleaseFile(format!(
      "Release file {} has an empty summary body.",
      source.display()
    )));
  }

  // Parse YAML frontmatter
  let yaml_value: serde_yaml::Value = serde_yaml::from_str(yaml_part).map_err(|e| {
    OxrlsError::ReleaseFile(format!(
      "Invalid YAML frontmatter in {}: {}",
      source.display(),
      e
    ))
  })?;

  let mapping = yaml_value.as_mapping().ok_or_else(|| {
    OxrlsError::ReleaseFile(format!(
      "Invalid frontmatter in {}: must be a mapping of package names to bump types.",
      source.display()
    ))
  })?;

  let mut releases: IndexMap<String, BumpType> = IndexMap::new();

  for (key, value) in mapping {
    let pkg_name = key
      .as_str()
      .ok_or_else(|| {
        OxrlsError::ReleaseFile(format!(
          "Invalid frontmatter in {}: package names must be strings.",
          source.display()
        ))
      })?
      .to_string();

    let bump_str = value.as_str().ok_or_else(|| {
      OxrlsError::ReleaseFile(format!(
        "Invalid bump type for package \"{}\" in {}: must be a string.",
        pkg_name,
        source.display()
      ))
    })?;

    let bump_type = bump_str
      .parse::<BumpType>()
      .map_err(|e| OxrlsError::ReleaseFile(format!("{} in {}", e, source.display())))?;

    releases.insert(pkg_name, bump_type);
  }

  if releases.is_empty() {
    return Err(OxrlsError::ReleaseFile(format!(
      "Release file {} has no package entries in frontmatter.",
      source.display()
    )));
  }

  Ok(ReleaseFile {
    path: source.to_path_buf(),
    releases,
    summary: summary.trim().to_string(),
  })
}

/// Parse frontmatter delimited by `---` markers.
/// Returns (yaml_content, body_content).
fn parse_frontmatter(content: &str) -> Option<(&str, &str)> {
  let content = content.trim();

  // Must start with "---"
  if !content.starts_with("---") {
    return None;
  }

  // Skip past the opening ---
  let after_opening = content[3..].trim_start();

  // The YAML content is everything before the closing "---" + newline
  // Find the closing "---" that appears on its own line
  // Look for "\n---" followed by end-of-line or newline
  let closing_marker = after_opening.find("\n---")?;

  // Split at the closing marker.
  // Trim trailing \r to handle CRLF line endings: the marker search splits on \n,
  // leaving \r at the end of the YAML content when the file uses \r\n.
  let yaml_content = after_opening[..closing_marker].trim_end_matches('\r');
  let after_closing = &after_opening[closing_marker + 4..]; // skip "\n---"
  let body = after_closing.trim_start();

  Some((yaml_content, body))
}

/// Generate a new release file path with a random adjective-noun name.
pub fn generate_release_filename(release_dir: &Path) -> PathBuf {
  let adjectives = [
    "ancient", "autumn", "blue", "bold", "brave", "bright", "calm", "cold", "cool", "crimson",
    "crystal", "dark", "dawn", "deep", "divine", "dry", "dusty", "eager", "early", "eastern",
    "electric", "empty", "faint", "fair", "falling", "fancy", "fast", "fat", "few", "fierce",
    "final", "first", "fixed", "flat", "flying", "free", "fresh", "frosty", "full", "gentle",
    "glad", "golden", "good", "grand", "gray", "great", "green", "happy", "hidden", "holy", "hot",
    "humble", "icy", "ideal", "inner", "iron", "jolly", "keen", "kind", "large", "last", "late",
    "leafy", "light", "little", "lively", "long", "loose", "loud", "low", "lucky", "lunar",
    "major", "mellow", "merry", "middle", "mighty", "mild", "mini", "minor", "misty", "mixed",
    "modern", "muddy", "mute", "narrow", "nasty", "native", "neat", "noble", "northern", "odd",
    "old", "open", "outer", "pale", "patient", "petite", "pink", "plain", "plastic", "playful",
    "plural", "polite", "poor", "pretty", "prompt", "proper", "proud", "pure", "purple", "quick",
    "quiet", "rainy", "rapid", "rare", "raw", "red", "rich", "rigid", "ripe", "rising", "rocky",
    "rough", "round", "royal", "rude", "rural", "sacred", "safe", "salty", "same", "second",
    "secure", "seven", "shallow", "sharp", "shining", "short", "shy", "silent", "simple", "single",
    "six", "skinny", "sleepy", "slim", "slow", "small", "smart", "smooth", "snug", "social",
    "soft", "solar", "solid", "sour", "spare", "spicy", "stable", "stale", "steady", "steep",
    "stern", "still", "stout", "strange", "strong", "sturdy", "subtle", "sudden", "sunny", "super",
    "sweet", "swift", "tall", "tame", "tasty", "tender", "tense", "thin", "three", "tight", "tiny",
    "tired", "tough", "trim", "true", "twin", "ugly", "unique", "upper", "upset", "urban", "used",
    "useful", "vague", "valid", "vast", "velvet", "vivid", "vocal", "warm", "wary", "waste",
    "watery", "wavy", "waxy", "weak", "weary", "weird", "western", "wet", "white", "whole", "wide",
    "wild", "windy", "winter", "wise", "witty", "wooden", "worthy", "young", "youthful", "zealous",
  ];

  let nouns = [
    "apple", "badger", "basket", "blossom", "bluebird", "breeze", "bridge", "brook", "bubble",
    "buckle", "butter", "button", "candle", "canyon", "castle", "cherry", "cloud", "clover",
    "coast", "comet", "coral", "cotton", "cradle", "crane", "creek", "crown", "crystal", "cub",
    "dawn", "deer", "dew", "diamond", "dolphin", "dragon", "dream", "dune", "dust", "eagle",
    "echo", "ember", "fawn", "feather", "fern", "field", "flame", "flower", "fog", "forest", "fox",
    "frost", "fruit", "galaxy", "garden", "gem", "glacier", "glade", "glow", "goat", "grape",
    "grass", "grove", "gull", "harbor", "hawk", "haze", "heather", "hill", "hollow", "honor",
    "horn", "horse", "humble", "icy", "ivy", "jade", "jeep", "journey", "jewel", "jungle", "koala",
    "lagoon", "lake", "lamb", "lamp", "lane", "lark", "leaf", "light", "lily", "lion", "lodge",
    "loom", "lotus", "lunar", "meadow", "metal", "mist", "moon", "moss", "moth", "mountain",
    "mouse", "mule", "oasis", "ocean", "opal", "orbit", "orchid", "osprey", "otter", "owl",
    "panda", "panel", "path", "pearl", "pebble", "petal", "pigeon", "pike", "pine", "pixel",
    "plant", "plateau", "plume", "pond", "prairie", "prism", "puma", "pyramid", "rabbit", "rain",
    "raven", "reed", "reef", "ripple", "river", "rivet", "robin", "rock", "roof", "rook", "rose",
    "ruby", "ruins", "saddle", "sage", "salmon", "sand", "satin", "saw", "scale", "sea", "seal",
    "seed", "shadow", "shard", "shell", "shield", "ship", "shrine", "silk", "silver", "skate",
    "sky", "sleet", "smoke", "snail", "snake", "snow", "solar", "spark", "spider", "spirit",
    "spoke", "spoon", "spring", "spruce", "square", "squirrel", "stag", "star", "steam", "steel",
    "stem", "stone", "storm", "stream", "summit", "sun", "surf", "swamp", "swan", "sweep", "swift",
    "swing", "talon", "temple", "thorn", "throne", "tide", "tiger", "timber", "toast", "token",
    "trail", "tray", "treasure", "tree", "tribe", "trick", "tulip", "tundra", "turbo", "turtle",
    "valley", "vapor", "velvet", "vine", "violet", "visor", "vista", "voice", "volcano", "vortex",
    "vowel", "wagon", "water", "wave", "whale", "wheel", "wild", "willow", "wind", "winter",
    "wish", "wolf", "wood", "wool", "world", "worm", "wren", "yacht", "yarn", "yield", "youth",
    "zeal", "zebra", "zenith",
  ];

  let mut rng = rand::thread_rng();
  let adj = adjectives.choose(&mut rng).unwrap_or(&"quiet");
  let noun = nouns.choose(&mut rng).unwrap_or(&"fox");
  let hash: String = (0..4)
    .map(|_| {
      let idx = rng.gen_range(0..16);
      format!("{:x}", idx)
    })
    .collect();

  let filename = format!("{}-{}-{}.md", hash, adj, noun);
  release_dir.join(filename)
}

/// Create a release markdown file.
pub fn create_release_file(
  release_dir: &Path,
  releases: &IndexMap<String, BumpType>,
  summary: &str,
  details: Option<&str>,
) -> Result<PathBuf> {
  fs::create_dir_all(release_dir).map_err(OxrlsError::Io)?;

  let path = generate_release_filename(release_dir);

  // Build YAML frontmatter
  let mut yaml_lines = String::new();
  for (pkg, bump) in releases {
    yaml_lines.push_str(&format!(
      "\"{}\": {}\n",
      pkg,
      serde_json::to_string(&bump).unwrap_or_else(|_| "patch".to_string())
    ));
  }

  let mut content = format!("---\n{}---\n\n{}", yaml_lines, summary);

  if let Some(details_text) = details
    && !details_text.is_empty()
  {
    // Indent each line of details by 2 spaces so it aligns under the summary
    let indented: String = details_text
      .lines()
      .map(|line| {
        if line.trim().is_empty() {
          "".to_string()
        } else {
          format!("  {}", line)
        }
      })
      .collect::<Vec<_>>()
      .join("\n");
    content.push_str(&format!("\n{}", indented));
  }

  fs::write(&path, content).map_err(OxrlsError::Io)?;
  Ok(path)
}

/// Remove a release file after successful bump.
pub fn consume_release_file(path: &Path) -> Result<()> {
  fs::remove_file(path).map_err(|e| {
    OxrlsError::ReleaseFile(format!(
      "Failed to remove release file {}: {}",
      path.display(),
      e
    ))
  })?;
  Ok(())
}

/// Archive a release file (move to archive subdirectory).
pub fn archive_release_file(path: &Path, archive_dir: &Path) -> Result<()> {
  fs::create_dir_all(archive_dir).map_err(OxrlsError::Io)?;
  let filename = path.file_name().ok_or_else(|| {
    OxrlsError::ReleaseFile(format!("Invalid release file path: {}", path.display()))
  })?;
  let dest = archive_dir.join(filename);
  fs::rename(path, &dest).map_err(|e| {
    OxrlsError::ReleaseFile(format!(
      "Failed to archive release file {}: {}",
      path.display(),
      e
    ))
  })?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;

  #[test]
  fn test_parse_valid_release_file() {
    let content = r#"---
"@scope/pkg-a": patch
"@scope/pkg-b": minor
---

Fixed editor selection behavior."#;

    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("test.md");
    std::fs::write(&path, content).unwrap();

    let rf = parse_release_file(&path).unwrap();
    assert_eq!(rf.releases.len(), 2);
    assert_eq!(rf.releases["@scope/pkg-a"], BumpType::Patch);
    assert_eq!(rf.releases["@scope/pkg-b"], BumpType::Minor);
    assert!(rf.summary.contains("Fixed editor selection"));
  }

  #[test]
  fn test_parse_empty_file() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("empty.md");
    std::fs::write(&path, "").unwrap();

    let result = parse_release_file(&path);
    assert!(result.is_err());
  }

  #[test]
  fn test_parse_no_frontmatter() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("nofm.md");
    std::fs::write(&path, "Just a summary").unwrap();

    let result = parse_release_file(&path);
    assert!(result.is_err());
  }

  #[test]
  fn test_parse_invalid_bump_type() {
    let content = r#"---
"@scope/pkg-a": feature
---

Summary here."#;
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("invalid.md");
    std::fs::write(&path, content).unwrap();

    let result = parse_release_file(&path);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("feature"));
  }

  #[test]
  fn test_bump_type_priority() {
    assert!(BumpType::Major.priority() > BumpType::Minor.priority());
    assert!(BumpType::Minor.priority() > BumpType::Patch.priority());
  }

  #[test]
  fn test_bump_type_max() {
    assert_eq!(
      BumpType::max(Some(BumpType::Patch), BumpType::Major),
      BumpType::Major
    );
    assert_eq!(
      BumpType::max(Some(BumpType::Major), BumpType::Patch),
      BumpType::Major
    );
    assert_eq!(BumpType::max(None, BumpType::Patch), BumpType::Patch);
    assert_eq!(
      BumpType::max(Some(BumpType::Minor), BumpType::Patch),
      BumpType::Minor
    );
  }

  #[test]
  fn test_generate_release_filename() {
    let tmp = TempDir::new().unwrap();
    let path = generate_release_filename(tmp.path());
    assert!(path.to_string_lossy().ends_with(".md"));
    let parent = path.parent().unwrap();
    assert_eq!(parent, tmp.path());
  }

  #[test]
  fn test_parse_frontmatter_crlf() {
    // CRLF line endings should not break frontmatter parsing
    let content = "---\r\n\"@scope/pkg-a\": patch\r\n---\r\n\r\nSummary with CRLF.\r\n";
    let (yaml, body) = parse_frontmatter(content).unwrap();
    assert!(
      !yaml.ends_with('\r'),
      "YAML should not have trailing CR: {:?}",
      yaml
    );
    assert_eq!(body, "Summary with CRLF.");
  }

  #[test]
  fn test_parse_release_crlf() {
    let content = "---\r\n\"@scope/pkg-a\": patch\r\n---\r\n\r\nSummary with CRLF.\r\n";
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("crlf.md");
    std::fs::write(&path, content).unwrap();

    let result = parse_release_file(&path).unwrap();
    assert_eq!(result.releases.len(), 1);
    assert_eq!(result.summary, "Summary with CRLF.");
    assert!(result.releases.contains_key("@scope/pkg-a"));
  }
}
