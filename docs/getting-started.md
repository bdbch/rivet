# Getting Started with oxrls

**oxrls** (short for *oxrelease*) is a Rust-powered Changesets-like release CLI for JavaScript/TypeScript monorepos. It helps you manage version bumps, changelogs, and internal dependency updates in a deterministic, safe way.

## Installation

oxrls is built as part of the `vp-release` crate. Build the binary:

```bash
cargo build --release
```

The binary is placed at `target/release/oxrls`. You can use it directly:

```bash
./target/release/oxrls --help
```

Or install it to your PATH:

```bash
cargo install --path .
```

## Quick start

### 1. Initialize oxrls in your project

```bash
oxrls init
```

This creates:

- `oxrls.json` — configuration file with sensible defaults
- `.oxrls/` — directory for pending release files
- `.oxrls/README.md` — informational readme

### 2. Create a release file

Interactive mode:

```bash
oxrls new
```

This prompts you to:
1. Select which packages changed (multi-select)
2. Choose the bump type for each (patch / minor / major)
3. Enter a summary of the change
4. Optionally add details

Non-interactive mode:

```bash
oxrls new --package @scope/core:patch --summary "Fix transaction mapping bug"
```

Multiple packages:

```bash
oxrls new \
  --package @scope/core:patch \
  --package @scope/react:minor \
  --summary "Improve editor behavior"
```

### 3. Check pending releases

```bash
oxrls status
```

Shows all pending release files and the calculated version bumps.

### 4. Apply the release

Dry run first (recommended):

```bash
oxrls bump --dry-run
```

Then apply:

```bash
oxrls bump
```

This:
1. Updates `package.json` versions for affected packages
2. Updates internal dependency ranges in dependent packages
3. Writes or appends `CHANGELOG.md` entries
4. Removes consumed release files

### Complete workflow example

```bash
# Initialize
oxrls init

# Create a release
oxrls new --package @scope/core:patch --summary "Fix transaction mapping bug"

# Review
oxrls status

# Apply (dry run first)
oxrls bump --dry-run
oxrls bump
```

## Dry run

Always safe to use `--dry-run` before a real bump:

```bash
oxrls bump --dry-run
```

This prints the full plan (which packages will be bumped, which dependency ranges will update, which files will be consumed) without writing anything.

## Archiving release files

By default, consumed release files are deleted. To archive them instead:

```bash
oxrls bump --archive
```

This moves files to `.oxrls/archive/` instead of deleting them.

## Exit codes

| Code | Meaning |
|------|---------|
| 0    | Success |
| 1    | Error (invalid input, validation failure, etc.) |

## Requirements

- Rust 2021 edition or later
- A monorepo with `package.json` workspaces or `pnpm-workspace.yaml`
- Node.js packages with valid `name` and `version` fields in `package.json`
