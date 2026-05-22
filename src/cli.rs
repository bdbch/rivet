use clap::{Parser, Subcommand};

/// oxrls — A Rust-powered Changesets-like release CLI for monorepos.
#[derive(Parser, Debug)]
#[command(
  name = "oxrls",
  version,
  about = "A Rust-powered release CLI for monorepos"
)]
pub struct Cli {
  #[command(subcommand)]
  pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
  /// Initialize oxrls configuration in the project
  Init {
    /// Overwrite existing config if present
    #[arg(long, default_value_t = false)]
    force: bool,

    /// Custom release directory
    #[arg(long)]
    release_dir: Option<String>,
  },

  /// Create a new release file
  New {
    /// Package and bump type (e.g., "@scope/pkg:patch")
    #[arg(long = "package", short = 'p')]
    packages: Vec<String>,

    /// Summary of the change
    #[arg(long)]
    summary: Option<String>,

    /// Optional details
    #[arg(long)]
    details: Option<String>,
  },

  /// Show pending release files and calculated bumps
  Status,

  /// Consume release files and apply version bumps
  Bump {
    /// Show what would happen without making changes
    #[arg(long, default_value_t = false)]
    dry_run: bool,

    /// Archive release files instead of deleting them
    #[arg(long, default_value_t = false)]
    archive: bool,
  },

  /// Publish all packages from the last bump to npm
  Release {
    /// Show what would be published without publishing
    #[arg(long, default_value_t = false)]
    dry_run: bool,

    /// Override the npm dist-tag (default: "latest" or pre-release tag)
    #[arg(long)]
    tag: Option<String>,
  },

  /// Manage pre-release mode for packages (omit subcommand for interactive mode)
  Pre {
    #[command(subcommand)]
    action: Option<PreAction>,
  },
}

#[derive(Subcommand, Debug)]
pub enum PreAction {
  /// Enter pre-release mode for packages with the given tag
  Enter {
    /// Pre-release tag (e.g., "beta", "alpha", "rc")
    #[arg(long)]
    tag: String,

    /// Package name or glob pattern (repeatable)
    #[arg(long = "package", short = 'p')]
    packages: Vec<String>,

    /// Force migration (move package from one tag to another)
    #[arg(long, default_value_t = false)]
    force: bool,
  },

  /// Exit pre-release mode for packages
  Exit {
    /// Package name or glob pattern (repeatable)
    #[arg(long = "package", short = 'p')]
    packages: Vec<String>,
  },

  /// Show pre-release status
  Status,
}
