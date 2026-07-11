# rivet

> A Rust-powered release CLI for JS/TS monorepos — version bumps, changelogs, pre-release mode, and npm publishing.

## Packages

This project contains two components:

| Component                     | Description                                                |
| ----------------------------- | ---------------------------------------------------------- |
| **NAPI addon** (`vp_release`) | Native Node.js addon built with [napi-rs](https://napi.rs) |
| **CLI** (`rivet`)             | Rust binary for release management                         |

## Quick start

```bash
npm install @bdbchgg/rivet --save-dev
```

Then add this to your `package.json` scripts:

```json
{
  "scripts": {
    "rivet": "rivet", # for convenience, you can run `npm run rivet` instead of `npx rivet`
    "changeset": "oxrl new", # create a new release file (similar to `changeset add`)
    "bump": "rivet bump", # apply version bumps, update deps, generate changelogs, similar to `changeset version`
    "release": "rivet release" # publish to npm (after bumping versions)
  }
}
```

```bash
# Initialize
rivet init

# Create a release file
rivet new --package @scope/core:patch --summary "Fix transaction mapping bug"

# Preview
rivet status
rivet bump --dry-run

# Pre-release mode (optional)
rivet pre # interactive, or
rivet pre enter alpha --package @scope/core # enter pre-release mode for @scope/core with "alpha" tag
rivet pre exit @scope/core # exit pre-release mode for @scope/core
# Alternatively, you can set pre-release mode in the config file for specific packages or groups, and it will be applied automatically when bumping versions.
# Just make sure to also remove the counter of the pre-released package in pre.json

# Apply
rivet bump

# Publish to npm
rivet release
```

## Vibecode Disclaimer

This project was vibecoded. I'm still learning Rust, and I use AI agents to review code, explain patterns, and help me understand what's going on. That said, I don't just blindly trust the output: I read through every change, I understand (most of) what the code does, and I make deliberate decisions. I also enforce TDD to make sure existing features don't break as the project evolves.

I'd love to get some eyes from the Rust community on best practices, patterns, and anything that could be improved. Pull requests, issues, and feedback are very welcome.

## Commands

| Command         | Description                                                  |
| --------------- | ------------------------------------------------------------ |
| `rivet init`    | Create `rivet.json` config and `.rivet/` directory (use `--force` to re-run) |
| `rivet new`     | Create a release file (interactive or `--package --summary`) |
| `rivet status`  | Show pending release files and calculated bumps              |
| `rivet bump`    | Apply version bumps, update deps, generate changelogs        |
| `rivet release` | Publish bumped packages to npm                               |
| `rivet pre`     | Manage pre-release mode (interactive or `enter`/`exit`)      |

## Documentation

See the full documentation at [rivet.dev](https://rivet.dev) or browse the [docs app](./apps/docs) in this repository.

## Build

```bash
cargo build --release
# binary at target/release/rivet
```

## NAPI Addon

The NAPI addon provides native Rust bindings for Node.js:

```bash
yarn build
yarn test
```
