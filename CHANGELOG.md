# Changelog

## 2026-05-22

### Major Changes

- **oxrls** (v1.0.0-alpha.1): First public release of oxrls — a Rust-powered release CLI for JS/TS monorepos.

  I'm incredibly happy to release this. oxrls has been a joy to build and I'm already using it for my own monorepos. It's fast, configurable, bendable and just works!
  
  Features included in this release:
  
  - `oxrls init` with interactive config wizard
  - `oxrls new` for creating release files (interactive and CLI mode)
  - `oxrls status` to preview pending changes
  - `oxrls bump` to apply version bumps, update deps, and generate changelogs
  - `oxrls release` to publish bumped packages to npm
  - `oxrls pre` for per-package pre-release mode (beta/alpha/rc)
  - Fixed and linked package groups with glob + negation pattern support
  - Workspace detection from package.json workspaces and pnpm-workspace.yaml
  - Internal dependency range updates with configurable strategy
  - Per-package and global CHANGELOG.md generation
  - SemVer-compliant version bumping
  - Deterministic read-compute-write safety model
  
  I hope this tool saves you as much time and headache as it's already saving me. ❤️
