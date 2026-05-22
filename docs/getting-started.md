# Getting Started with oxrls

**oxrls** (short for *oxrelease*) is a Rust-powered release management CLI for JavaScript/TypeScript monorepos. It handles version bumps, changelogs, internal dependency updates, pre-release versions, and npm publishing.

## Installation

Build from source:

```bash
cargo build --release
```

The binary is at `target/release/oxrls`. Install to PATH:

```bash
cargo install --path .
```

## Quick start

### 1. Initialize

```bash
oxrls init
```

Creates `.oxrls/config.json` with defaults and a `.oxrls/` directory.

### 2. Create a release file

```bash
# Interactive — select packages, choose bump type, write summary
oxrls new

# Non-interactive
oxrls new --package @scope/core:patch --summary "Fix transaction mapping bug"

# Multiple packages
oxrls new \
  --package @scope/core:patch \
  --package @scope/react:minor \
  --summary "Improve editor behavior"
```

### 3. Preview and apply

```bash
oxrls status               # shows pending release files and calculated bumps
oxrls bump --dry-run        # preview without writing
oxrls bump                  # apply version bumps, update deps, generate changelogs
```

### 4. Publish

```bash
oxrls release               # publish all bumped packages to npm
oxrls release --dry-run     # preview without publishing
oxrls release --tag beta    # publish with a custom npm dist-tag
```

## Example workflow

```bash
oxrls init

# Record changes
oxrls new --package @scope/core:patch --summary "Fix transaction mapping bug"

# Apply
oxrls status
oxrls bump --dry-run
oxrls bump

# Publish
oxrls release
```

After `oxrls bump`:
- Package versions are updated in `package.json`
- Internal dependency ranges are updated
- `CHANGELOG.md` is created/updated
- Release files are consumed
- A `.oxrls/releaseplan.txt` is written for `oxrls release`

## Workspace detection

oxrls auto-detects workspaces from:

- `package.json` workspaces (array or object format)
- `pnpm-workspace.yaml`
- No config = single-package mode (root is the only package)

## Requirements

- Rust 2021 edition
- Node.js project with `package.json`
- `npm` for publishing
