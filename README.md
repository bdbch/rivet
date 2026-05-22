# oxrls

> A Rust-powered native addon (napi-rs) + a Changesets-like release CLI for JS/TS monorepos.

## Packages

This project contains two components:

| Component | Description |
|-----------|-------------|
| **NAPI addon** (`vp_release`) | Native Node.js addon built with [napi-rs](https://napi.rs) |
| **CLI** (`oxrls`) | Rust binary for release management — version bumps, changelogs, and dependency updates |

## oxrls — Release CLI

`oxrls` is a deterministic, safe release management tool for JavaScript/TypeScript monorepos. Think of it as "Changesets in Rust" — faster, stricter, and more predictable.

### Quick start

```bash
# Initialize config
oxrls init

# Create a release file
oxrls new --package @scope/core:patch --summary "Fix transaction mapping bug"

# Check pending releases
oxrls status

# Apply bumps (dry run first)
oxrls bump --dry-run
oxrls bump
```

### Commands

| Command | Description |
|---------|-------------|
| `oxrls init` | Create `oxrls.json` config and release directory |
| `oxrls new` | Create a markdown release file (interactive or CLI flags) |
| `oxrls status` | Show pending release files and calculated bumps |
| `oxrls bump` | Apply version bumps, update deps, generate changelogs |

### Documentation

For detailed documentation, see the [docs/](./docs/) directory:

- [Getting Started](./docs/getting-started.md) — installation, quick start, workflow
- [Configuration](./docs/configuration.md) — config file reference
- [Usage](./docs/usage.md) — command reference, release file format, internals

### Example workflow

```bash
# Initialize in your project
oxrls init

# Record a change
oxrls new --package @scope/core:patch --summary "Fix transaction mapping bug"

# Preview what will happen
oxrls status
oxrls bump --dry-run

# Apply the release
oxrls bump
```

After `oxrls bump`:
- `@scope/core` version goes from `1.2.3` → `1.2.4`
- Any workspace package depending on `@scope/core` gets its dependency range updated
- `CHANGELOG.md` is created/updated for `@scope/core`
- The release file is consumed

---

## NAPI Addon

The NAPI addon provides native Rust bindings for Node.js, built with `napi-rs`.

### Build

```bash
yarn build
# or
npm run build
```

### Test

```bash
yarn test
# or
npm test
```

### Develop requirements

- Install the latest [Rust](https://rustup.rs/)
- Install Node.js 18+ which fully supports Node-API
- Install yarn 4.x

### Release

Ensure you have set your **NPM_TOKEN** in the GitHub project settings.

```bash
npm version [<newversion> | major | minor | patch]
git push
```

GitHub Actions will build and publish the native packages.
