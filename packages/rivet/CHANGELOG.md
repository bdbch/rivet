# Changelog

## 2026-05-23

### Patch Changes

- **@bdbchgg/rivet** (v1.0.0-alpha.5): Fixed a bug where a pre-version would not correctly bump it's version on the first pre-version

## 2026-05-23

### Patch Changes

- **@bdbchgg/rivet** (v1.0.0-alpha.4): Fixed a bug that caused the binary not to be included with the bundle

## 2026-05-22

### Patch Changes

- **@bdbchgg/rivet** (v1.0.0-alpha.3): Improve CLI output formatting with aligned columns and unicode arrows

## 2026-05-22

### Patch Changes

- **@bdbchgg/rivet** (v1.0.0-alpha.2): Fix: rivet status no longer increments pre-release counters
- **@bdbchgg/rivet** (v1.0.0-alpha.2): Move config to .rivet/config.json, fix release_dir_abs for nested config
- **@bdbchgg/rivet** (v1.0.0-alpha.2): Fix: dry-run no longer increments pre-release counters
- **@bdbchgg/rivet** (v1.0.0-alpha.2): Fix: pre-release keeps base version, only increments counter
- **@bdbchgg/rivet** (v1.0.0-alpha.2): Fixed a bug where release files with multiple lines would not be indented correctly on changelogs
- **@bdbchgg/rivet** (v1.0.0-alpha.2): Add rivet check command for CI pipelines
- **@bdbchgg/rivet** (v1.0.0-alpha.2): Add hex prefix to release filenames and syncCargoToml config option

## 2026-05-22

### Major Changes

- **@bdbchgg/rivet** (v1.0.0-alpha.1): First public release of rivet — a Rust-powered release CLI for JS/TS monorepos.

  I'm incredibly happy to release this. rivet has been a joy to build and I'm already using it for my own monorepos. It's fast, configurable, bendable and just works!

  Features included in this release:
  - `rivet init` with interactive config wizard
  - `rivet new` for creating release files (interactive and CLI mode)
  - `rivet status` to preview pending changes
  - `rivet bump` to apply version bumps, update deps, and generate changelogs
  - `rivet release` to publish bumped packages to npm
  - `rivet pre` for per-package pre-release mode (beta/alpha/rc)
  - Fixed and linked package groups with glob + negation pattern support
  - Workspace detection from package.json workspaces and pnpm-workspace.yaml
  - Internal dependency range updates with configurable strategy
  - Per-package and global CHANGELOG.md generation
  - SemVer-compliant version bumping
  - Deterministic read-compute-write safety model

  I hope this tool saves you as much time and headache as it's already saving me. ❤️
