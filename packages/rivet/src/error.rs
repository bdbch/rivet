use thiserror::Error;

#[derive(Error, Debug)]
pub enum RivetError {
  #[error("Config error: {0}")]
  Config(String),

  #[error("Workspace error: {0}")]
  Workspace(String),

  #[error("Release file error: {0}")]
  ReleaseFile(String),

  #[error("Package error: {0}")]
  Package(String),

  #[error("Version error: {0}")]
  Version(String),

  #[error("Changelog error: {0}")]
  Changelog(String),

  #[error("Bump error: {0}")]
  Bump(String),

  #[error("IO error: {0}")]
  Io(#[from] std::io::Error),

  #[error("JSON error: {0}")]
  Json(#[from] serde_json::Error),

  #[error("YAML error: {0}")]
  Yaml(#[from] serde_yaml::Error),

  #[error("Semver error: {0}")]
  Semver(#[from] semver::Error),

  #[error("CLI error: {0}")]
  Cli(String),

  #[error("{0}")]
  Other(String),
}

pub type Result<T> = std::result::Result<T, RivetError>;

impl From<String> for RivetError {
  fn from(s: String) -> Self {
    RivetError::Other(s)
  }
}

impl From<&str> for RivetError {
  fn from(s: &str) -> Self {
    RivetError::Other(s.to_string())
  }
}
