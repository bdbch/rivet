# Configuration

oxrls looks for `.oxrls/config.json` or `..oxrls/config.json` in the project root or any parent directory.

## Default config

```json
{
  "$schema": "https://oxrelease.dev/schema.json",
  "releaseDir": ".oxrls",
  "changelog": true,
  "generatePackagesChangelog": true,
  "generateGlobalChangelog": false,
  "updateInternalDependencies": "patch",
  "baseBranch": "main",
  "access": "public",
  "fixed": [],
  "linked": [],
  "preMode": []
}
```

## Options

### `releaseDir`

Default: `".oxrls"`

```json
{ "releaseDir": ".oxrls" }
```

### `changelog` (legacy)

Default: `true`

Setting this to `false` disables all changelog generation, regardless of the new flags below.

### `generatePackagesChangelog`

Default: `true`

Generate individual `CHANGELOG.md` files per workspace package.

```json
{ "generatePackagesChangelog": false }
```

### `generateGlobalChangelog`

Default: `false`

Generate a single `CHANGELOG.md` at the project root aggregating all package changes.

```json
{ "generateGlobalChangelog": true }
```

**Solo repo fallback**: if the workspace has only one package and per-package changelogs are enabled, oxrls automatically generates a global changelog instead.

### `updateInternalDependencies`

Default: `"patch"`

Controls when internal dependency ranges are updated:

| Value | Behavior |
|-------|----------|
| `"always"` | Always update ranges |
| `"patch"` | Update when dependency got at least a patch |
| `"minor"` | Update only for minor or major |
| `"major"` | Update only for major |
| `"never"` | Never update |

### `baseBranch`

Default: `"main"`

### `access`

Default: `"public"`

Either `"public"` or `"restricted"`. Overridable per-package via `publishConfig.access` in `package.json`.

### `fixed`

Default: `[]`

Groups of packages that **always share the same version**. When any member is bumped, all members get bumped to the same new version (highest old version + max bump type).

Supports glob patterns and `!` negation:

```json
{
  "fixed": [
    ["@scope/design-system", "@scope/theme"],
    ["@scope/*", "!@scope/standalone"]
  ]
}
```

### `linked`

Default: `[]`

Groups of packages that **share the same bump type**. When a member receives a bump, all members in the group get the highest bump type found in the group.

Supports glob patterns and `!` negation:

```json
{
  "linked": [
    ["@scope/hooks", "@scope/utils"]
  ]
}
```

### `preMode`

Default: `[]`

Pre-release mode configuration. Packages listed here produce pre-release versions with a tag suffix.

```json
{
  "preMode": [
    {
      "tag": "beta",
      "packages": ["@scope/experimental-*", "@scope/new-feature"]
    },
    {
      "tag": "alpha",
      "packages": ["@scope/early-access-*"]
    }
  ]
}
```

**Per-package granularity**: different packages can have different pre-release tags. A package can only be in one pre-mode at a time.

**Version behavior:**
| Scenario | Result |
|----------|--------|
| `1.2.3` + patch + beta | `1.2.4-beta.1` |
| `1.2.3` + major + rc | `2.0.0-rc.1` |
| Bump again in beta | `1.2.4-beta.2` |
| Exit pre-mode | `1.2.4` (normal) |
| Migrate beta → rc | counter resets → `2.0.0-rc.1` |

**State file**: `.oxrls/pre.json` tracks per-package counters. Auto-managed during `oxrls bump`.

## Creating config

```bash
oxrls init
oxrls init --release-dir .changes
oxrls init --force           # overwrite existing
```
