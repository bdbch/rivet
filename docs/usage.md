# Usage

## Commands

### `oxrls init`

```bash
oxrls init [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--force` | Overwrite existing config |
| `--release-dir <DIR>` | Custom release directory (default: `.oxrls`) |
| `--non-interactive` | Skip the config wizard and use defaults |

**Interactive mode** (default): walks you through all config options step by step:

1. Release directory
2. Changelog preferences (per-package, global, or none)
3. Base branch
4. Internal dependency update strategy
5. Default npm access
6. Linked package groups (monorepo only)
7. Fixed package groups (monorepo only)

**Non-interactive mode** (`--non-interactive`): creates config with defaults, same as before.

---

### `oxrls new`

```bash
oxrls new [OPTIONS]
```

**Interactive mode** (no flags): select packages, choose one bump type for all, enter summary + optional details.

**Non-interactive mode:**

```bash
oxrls new --package @scope/core:patch --summary "Fix bug"
oxrls new \
  --package @scope/core:patch \
  --package @scope/react:minor \
  --summary "Improve editor behavior"
```

**Options:**

| Option | Description |
|--------|-------------|
| `-p`, `--package <PKG:TYPE>` | Package + bump type (repeatable) |
| `--summary <TEXT>` | Summary of the change |
| `--details <TEXT>` | Optional body text |

**Generated file:**

```markdown
---
"@scope/core": patch
"@scope/react": minor
---

Improve editor behavior.
```

Files use random adjective-noun names like `calm-blue-fox.md`.

---

### `oxrls status`

```bash
oxrls status
```

Shows pending release files, calculated version bumps, and pre-release status.

---

### `oxrls bump`

```bash
oxrls bump [--dry-run] [--archive]
```

Consumes all pending release files and applies version bumps:

1. Read and validate all release files
2. Merge bump types (major > minor > patch)
3. Apply fixed/linked group constraints
4. Compute new versions (with pre-release tags if configured)
5. Update `package.json` versions
6. Update internal dependency ranges
7. Generate or update `CHANGELOG.md` files
8. Save release plan for `oxrls release`
9. Consume release files

**Safety**: read everything → validate → compute plan → write files. If anything fails, nothing is written.

---

### `oxrls release`

```bash
oxrls release [--dry-run] [--tag <TAG>]
```

Publishes all packages from the last successful `oxrls bump` to npm.

Reads `.oxrls/releaseplan.txt` (a plain text list of package names, one per line, alphabetically sorted).

**Behavior:**
- **Private packages** (`"private": true`) are skipped
- **Version check**: verifies `package.json` version matches the expected version
- **Exists check**: runs `npm view <pkg>@<version> version` first — skips if already published
- **Dist-tag**: `--tag` override → pre-release tag from version → `"latest"`
- **Access**: reads `publishConfig.access` from `package.json` → falls back to `.oxrls/config.json` config
- **Registry**: reads `publishConfig.registry` from `package.json` if set
- After success, the manifest file is removed

```bash
oxrls release --dry-run        # preview
oxrls release                   # publish everything
oxrls release --tag next        # override dist-tag for all packages
```

---

### `oxrls pre`

```bash
oxrls pre [SUBCOMMAND]
```

**Interactive mode** (no subcommand): select packages from a list, then enter a tag name (defaults to `beta`).

**Subcommands:**

```bash
# Enter pre-release mode
oxrls pre enter beta --package @scope/pkg-c
oxrls pre enter beta --package @scope/pkg-c --package @scope/pkg-d
oxrls pre enter beta --package "@scope/pre-*"
oxrls pre enter rc --package @scope/pkg-c --force    # migrate tag

# Exit pre-release mode
oxrls pre exit --package @scope/pkg-c
oxrls pre exit --package "@scope/pre-*"

# Show status
oxrls pre status
```

Package names support:
- Exact names: `@scope/pkg-c`
- Partial names: `pkg-c` resolves to `@scope/pkg-c` by suffix matching
- Globs: `"@scope/pre-*"`
- Negation: `"!@scope/special"` (in config)

---

## Release file format

```markdown
---
"@scope/pkg-a": patch
"@scope/pkg-b": minor
---

Summary of changes.
```

- Frontmatter is required (delimited by `---`)
- Bump types: `patch`, `minor`, `major`
- Body must not be empty
- Unknown packages produce a clear error

---

## Version bump rules

| Current | Bump | Result |
|---------|------|--------|
| 1.2.3 | patch | 1.2.4 |
| 1.2.3 | minor | 1.3.0 |
| 1.2.3 | major | 2.0.0 |
| 0.2.3 | major | 1.0.0 |
| 0.2.3 | minor | 0.3.0 |
| 0.2.3 | patch | 0.2.4 |

Multiple bumps for same package: **major > minor > patch**.

---

## Fixed packages

All packages in a fixed group share the same version:

```json
{ "fixed": [["@scope/core", "@scope/utils"]] }
```

If `@scope/core` gets a bump, both packages get the same new version (highest old version + max bump type).

---

## Linked packages

All packages in a linked group share the same bump type:

```json
{ "linked": [["@scope/hooks", "@scope/utils"]] }
```

If one gets `minor`, all get `minor`. Each keeps its own version number.

---

## Internal dependency updates

oxrls updates dependency ranges in `dependencies`, `devDependencies`, `peerDependencies`, and `optionalDependencies`:

| Original | Updated |
|----------|---------|
| `^1.2.3` | `^1.2.4` |
| `~1.2.3` | `~1.2.4` |
| `1.2.3` | `1.2.4` |
| `workspace:*` | `workspace:*` |
| `workspace:^` | `workspace:^` |
| `workspace:~` | `workspace:~` |
| `workspace:^1.2.3` | `workspace:^1.2.4` |

---

## Changelog format

### Per-package (default)

```markdown
# Changelog

## 1.2.4

### Patch Changes

- Fixed editor selection behavior.
```

### Global (opt-in via `generateGlobalChangelog`)

```markdown
# Changelog

## 2026-05-22

### Minor Changes

- **@scope/react** (v1.1.0): Add new feature.

### Patch Changes

- **@scope/core** (v1.2.4): Fix transaction mapping bug.
- **@scope/utils** (v1.2.4): Updated with @scope/core.
```

---

## Pre-release mode

Controlled via config or CLI:

```bash
oxrls pre enter beta --package "@scope/experimental-*"
oxrls pre                     # interactive
```

Version examples:

| Bump | Without pre | With pre (beta) |
|------|-------------|-----------------|
| patch | `1.2.4` | `1.2.4-beta.1` |
| minor | `1.3.0` | `1.3.0-beta.1` |
| major | `2.0.0` | `2.0.0-beta.1` |

Pre-release tag is used as the npm dist-tag during `oxrls release`:

```bash
oxrls release          # publishes with --tag beta
oxrls release --tag next  # overrides to --tag next
```

---

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error |
