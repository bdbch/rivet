# Configuration

oxrls looks for a config file named `oxrls.json` or `.oxrls.json` in the project root or any parent directory.

## Default config

```json
{
  "$schema": "https://oxrelease.dev/schema.json",
  "releaseDir": ".oxrls",
  "changelog": true,
  "updateInternalDependencies": "patch",
  "baseBranch": "main",
  "access": "public"
}
```

## Options

### `releaseDir`

Default: `".oxrls"`

The directory where pending release files are stored (relative to the project root).

```json
{
  "releaseDir": ".oxrls"
}
```

---

### `changelog`

Default: `true`

Whether to generate and update `CHANGELOG.md` files when bumping.

```json
{
  "changelog": false
}
```

---

### `updateInternalDependencies`

Default: `"patch"`

Controls when internal workspace dependency ranges are updated when a dependency package is bumped.

| Value | Behavior |
|-------|----------|
| `"always"` | Always update ranges |
| `"patch"` | Update when the dependency got at least a patch bump |
| `"minor"` | Update only for minor or major bumps |
| `"major"` | Update only for major bumps |
| `"never"` | Never update ranges |

```json
{
  "updateInternalDependencies": "minor"
}
```

---

### `baseBranch`

Default: `"main"`

The base branch used for branching and release metadata.

```json
{
  "baseBranch": "main"
}
```

---

### `access`

Default: `"public""

The npm access level for publishing. Either `"public"` or `"restricted"`.

```json
{
  "access": "restricted"
}
```

## Config file lookup

oxrls searches for config in this order:

1. `oxrls.json` in the current directory or any parent
2. `.oxrls.json` in the current directory or any parent
3. If neither is found, defaults are used

## Creating config

Use `oxrls init` to create a config file with defaults:

```bash
oxrls init
```

Override defaults:

```bash
oxrls init --release-dir .changes
```

Force overwrite an existing config:

```bash
oxrls init --force
```
