# Usage

## Commands

### `oxrls init`

Initializes oxrls configuration in the current project.

```bash
oxrls init [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--force` | Overwrite existing config if present |
| `--release-dir <DIR>` | Custom release directory name (default: `.oxrls`) |

**What it creates:**

- `oxrls.json` — project configuration
- `.oxrls/README.md` — informational readme inside the release directory

---

### `oxrls new`

Creates a new release file in the release directory.

```bash
oxrls new [OPTIONS]
```

**Interactive mode** (no options):

Runs an interactive prompt that walks you through:
1. Selecting workspace packages
2. Choosing bump types (patch / minor / major) for each
3. Writing a summary
4. Optionally adding details

**Non-interactive mode:**

```bash
oxrls new --package @scope/core:patch --summary "Fix bug"
```

**Options:**

| Option | Description |
|--------|-------------|
| `-p`, `--package <PKG:TYPE>` | Package name and bump type (can be repeated) |
| `--summary <TEXT>` | Summary of the change |
| `--details <TEXT>` | Optional details/body text |

**Examples:**

Single package:

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

**Generated file format:**

```markdown
---
"@scope/core": patch
"@scope/react": minor
---

Improve editor behavior.
```

Files are named with a random adjective-noun pattern, e.g., `calm-blue-fox.md`.

---

### `oxrls status`

Shows all pending release files and the calculated version bumps.

```bash
oxrls status
```

**Example output:**

```
Pending release files:

  .oxrls/calm-blue-fox.md
    @scope/core  patch
    @scope/react  minor

Calculated bumps:

  @scope/core  1.2.3 -> 1.2.4
  @scope/react  0.4.0 -> 0.5.0
```

**Exit codes:**

| Code | Meaning |
|------|---------|
| 0    | Valid release files, bumps computable |
| 1    | Invalid files or errors |

---

### `oxrls bump`

Consumes all pending release files and applies version bumps.

```bash
oxrls bump [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--dry-run` | Show what would happen without writing |
| `--archive` | Archive consumed files instead of deleting |

**What it does:**

1. Reads and validates all pending release files
2. Merges bump types per package (major > minor > patch)
3. Calculates new versions
4. Updates `package.json` versions for affected packages
5. Updates internal dependency ranges in dependent packages
6. Generates or updates `CHANGELOG.md` entries
7. Removes (or archives) consumed release files

**Example output:**

```
Bumped packages:

  @scope/core  1.2.3 -> 1.2.4
  @scope/react  1.0.0 -> 1.1.0

Updated internal dependencies:

  packages/react/package.json
    @scope/core ^1.2.3 -> ^1.2.4

Consumed release files:

  .oxrls/calm-blue-fox.md

Done!
```

**Safety:**

oxrls follows a read-compute-write cycle:

1. Read all release files and workspace state
2. Validate everything (package names, versions, bump types)
3. Compute the full release plan in memory
4. Write package.json files
5. Write changelogs
6. Remove release files last

If any validation fails, **no files are written**.

---

## Release file format

Release files are markdown files with YAML frontmatter:

```markdown
---
"@scope/package-a": patch
"@scope/package-b": minor
---

Fixed editor selection behavior when replacing nested nodes.
```

**Rules:**

- Frontmatter is required (delimited by `---`)
- Package names are quoted strings as keys
- Bump types must be `patch`, `minor`, or `major`
- Body (summary) must not be empty
- Unknown packages produce a clear error
- Invalid files are not partially consumed

---

## Workspace support

oxrls detects workspace packages from:

### package.json workspaces (array format)

```json
{
  "workspaces": ["packages/*", "apps/*"]
}
```

### package.json workspaces (object format)

```json
{
  "workspaces": {
    "packages": ["packages/*", "apps/*"]
  }
}
```

### pnpm-workspace.yaml

```yaml
packages:
  - "packages/*"
  - "apps/*"
```

Each workspace package must have a `name` and `version` in its `package.json`.

---

## Version bump rules

oxrls follows standard SemVer:

| Current | Bump type | Result |
|---------|-----------|--------|
| 1.2.3   | patch     | 1.2.4  |
| 1.2.3   | minor     | 1.3.0  |
| 1.2.3   | major     | 2.0.0  |
| 0.2.3   | major     | 1.0.0  |
| 0.2.3   | minor     | 0.3.0  |
| 0.2.3   | patch     | 0.2.4  |

When multiple release files affect the same package, the highest-priority bump wins:

```
major > minor > patch
```

---

## Internal dependency updates

When a workspace package version changes, oxrls updates local dependents' dependency ranges.

**Scanned fields:**

- `dependencies`
- `devDependencies`
- `peerDependencies`
- `optionalDependencies`

**Supported range formats:**

| Original range | Updated range |
|----------------|---------------|
| `^1.2.3` | `^1.2.4` |
| `~1.2.3` | `~1.2.4` |
| `1.2.3` | `1.2.4` |
| `workspace:*` | `workspace:*` (unchanged) |
| `workspace:^` | `workspace:^` (unchanged) |
| `workspace:~` | `workspace:~` (unchanged) |
| `workspace:^1.2.3` | `workspace:^1.2.4` |
| `workspace:~1.2.3` | `workspace:~1.2.4` |
| `workspace:1.2.3` | `workspace:1.2.4` |

---

## Changelog format

Generated changelogs follow this structure:

```markdown
# @scope/package-a

## 1.2.4

### Patch Changes

- Fixed editor selection behavior when replacing nested nodes.

## 1.2.3

### Patch Changes

- Previous fix.
```

Changes are grouped by type:

```
### Major Changes
### Minor Changes
### Patch Changes
```

Existing changelog content is preserved below the new version entry.
