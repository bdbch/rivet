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
    "bump": "oxrls bump", # apply version bumps, update deps, generate changelogs, similar to `changeset version`
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

# Pre-release mode (optional)
oxrls pre # interactive, or
oxrls pre enter alpha --package @scope/core # enter pre-release mode for @scope/core with "alpha" tag
oxrls pre exit @scope/core # exit pre-release mode for @scope/core
# Alternatively, you can set pre-release mode in the config file for specific packages or groups, and it will be applied automatically when bumping versions.
# Just make sure to also remove the counter of the pre-released package in pre.json

# Apply
oxrls bump

# Publish to npm
oxrls release
```

## Vibecode Disclaimer

This project was vibecoded. I'm still learning Rust, and I use AI agents to review code, explain patterns, and help me understand what's going on. That said, I don't just blindly trust the output: I read through every change, I understand (most of) what the code does, and I make deliberate decisions. I also enforce TDD to make sure existing features don't break as the project evolves.

I'd love to get some eyes from the Rust community on best practices, patterns, and anything that could be improved. Pull requests, issues, and feedback are very welcome.

## Commands

| Command         | Description                                                  |
| --------------- | ------------------------------------------------------------ |
| `oxrls init`    | Create `oxrls.json` config and `.oxrls/` directory (use `--force` to re-run) |
| `oxrls new`     | Create a release file (interactive or `--package --summary`) |
| `oxrls status`  | Show pending release files and calculated bumps              |
| `oxrls bump`    | Apply version bumps, update deps, generate changelogs        |
| `oxrls release` | Publish bumped packages to npm                               |
| `oxrls pre`     | Manage pre-release mode (interactive or `enter`/`exit`)      |

## Documentation

See the full documentation at [oxrls.dev](https://oxrls.dev) or browse the [docs app](./apps/docs) in this repository.

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
