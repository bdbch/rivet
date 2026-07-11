use crate::release_file::BumpType;
use semver::Version;

/// Apply a version bump according to SemVer rules.
pub fn bump_version(version: &Version, bump_type: BumpType) -> Version {
  match bump_type {
    BumpType::Patch => {
      let mut v = version.clone();
      v.patch += 1;
      v
    }
    BumpType::Minor => Version::new(version.major, version.minor + 1, 0),
    BumpType::Major => Version::new(version.major + 1, 0, 0),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_patch_bump() {
    let v = Version::new(1, 2, 3);
    assert_eq!(bump_version(&v, BumpType::Patch), Version::new(1, 2, 4));
  }

  #[test]
  fn test_minor_bump() {
    let v = Version::new(1, 2, 3);
    assert_eq!(bump_version(&v, BumpType::Minor), Version::new(1, 3, 0));
  }

  #[test]
  fn test_major_bump() {
    let v = Version::new(1, 2, 3);
    assert_eq!(bump_version(&v, BumpType::Major), Version::new(2, 0, 0));
  }

  #[test]
  fn test_major_bump_zero_major() {
    let v = Version::new(0, 2, 3);
    assert_eq!(bump_version(&v, BumpType::Major), Version::new(1, 0, 0));
  }

  #[test]
  fn test_minor_bump_zero_major() {
    let v = Version::new(0, 2, 3);
    assert_eq!(bump_version(&v, BumpType::Minor), Version::new(0, 3, 0));
  }

  #[test]
  fn test_patch_bump_zero_major() {
    let v = Version::new(0, 2, 3);
    assert_eq!(bump_version(&v, BumpType::Patch), Version::new(0, 2, 4));
  }

  #[test]
  fn test_consecutive_bumps() {
    let v = Version::new(1, 0, 0);
    let v = bump_version(&v, BumpType::Patch);
    assert_eq!(v, Version::new(1, 0, 1));
    let v = bump_version(&v, BumpType::Minor);
    assert_eq!(v, Version::new(1, 1, 0));
    let v = bump_version(&v, BumpType::Major);
    assert_eq!(v, Version::new(2, 0, 0));
  }
}
