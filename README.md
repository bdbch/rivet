# oxrls

> A Rust-powered release CLI for JS/TS monorepos — version bumps, changelogs, pre-release mode, and npm publishing.

## Packages

This project contains two components:

| Component                     | Description                                                |
| ----------------------------- | ---------------------------------------------------------- |
| **NAPI addon** (`vp_release`) | Native Node.js addon built with [napi-rs](https://napi.rs) |
| **CLI** (`oxrls`)             | Rust binary for release management                         |

## Quick start

```bash
npm install @bdbchgg/oxrls --save-dev
```

Then add this to your `package.json` scripts:

```json
{
  "scripts": {
    "oxrls": "oxrls", # for convenience, you can run `npm run oxrls` instead of `npx oxrls`
    "changeset": "oxrl new", # create a new release file (similar to `changeset add`)
    "release": "oxrls release" # publish to npm (after bumping versions)
  }
}
```

```bash
# Initialize
oxrls init

# Create a release file
oxrls new --package @scope/core:patch --summary "Fix transaction mapping bug"

# Preview
oxrls status
oxrls bump --dry-run

# Apply
oxrls bump

# Publish to npm
oxrls release
```

## Commands

| Command         | Description                                                  |
| --------------- | ------------------------------------------------------------ |
| `oxrls init`    | Create `oxrls.json` config and `.oxrls/` directory           |
| `oxrls new`     | Create a release file (interactive or `--package --summary`) |
| `oxrls status`  | Show pending release files and calculated bumps              |
| `oxrls bump`    | Apply version bumps, update deps, generate changelogs        |
| `oxrls release` | Publish bumped packages to npm                               |
| `oxrls pre`     | Manage pre-release mode (interactive or `enter`/`exit`)      |

## Documentation

See the [docs/](./docs/) directory:

- [Getting Started](./docs/getting-started.md) — installation, quick start, workflow
- [Configuration](./docs/configuration.md) — all config options (changelog, pre-mode, fixed/linked groups)
- [Usage](./docs/usage.md) — full command reference, release files, version rules, internals

## Build

```bash
cargo build --release
# binary at target/release/oxrls
```

## NAPI Addon

The NAPI addon provides native Rust bindings for Node.js:

```bash
yarn build
yarn test
```
