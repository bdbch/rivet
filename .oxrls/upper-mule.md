---
"oxrls": "major"
---

First public release of oxrls — a Rust-powered release CLI for JS/TS monorepos.

After weeks of building, I'm incredibly happy to finally release this. oxrls has been a joy to build and I'm already using it for my own monorepos. It's faster, stricter, and more deterministic than anything I've used before.

Features included in this release:

- oxrls init with interactive config wizard
- oxrls new for creating release files (interactive and CLI mode)
- oxrls status to preview pending changes
- oxrls bump to apply version bumps, update deps, and generate changelogs
- oxrls release to publish bumped packages to npm
- oxrls pre for per-package pre-release mode (beta/alpha/rc)
- Fixed and linked package groups with glob + negation pattern support
- Workspace detection from package.json workspaces and pnpm-workspace.yaml
- Internal dependency range updates with configurable strategy
- Per-package and global CHANGELOG.md generation
- SemVer-compliant version bumping
- Deterministic read-compute-write safety model

I hope this tool saves you as much time and headache as it's already saving me. ❤️