# Rivet GitHub Action

The action creates or updates a version pull request when `.rivet/` contains release files. The pull request body is automatically generated from the changelog entries so reviewers can see all changes at a glance. After the pull request is merged, the action sees `.rivet/releaseplan.txt` and runs the configured publish command.

## Workflow

```yaml
name: Rivet Release

on:
  push:
    branches: [main]

permissions:
  contents: write
  pull-requests: write

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: pnpm/action-setup@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 24
          cache: pnpm

      - run: pnpm install --frozen-lockfile

      - uses: bdbch/rivet@v1
        with:
          # Build native artifacts before `rivet release` in real projects.
          publish: pnpm build && pnpm exec rivet release
        env:
          NPM_TOKEN: ${{ secrets.NPM_TOKEN }}
```

The action itself cannot infer how a consuming monorepo builds native artifacts. Use the `publish` input to provide that project-specific command. The `github-token` needs `contents: write` and `pull-requests: write` permissions.

The default commands are:

- `pnpm exec rivet check --json`
- `pnpm exec rivet bump`
- no publish command unless `publish` is configured

By default, the Action uses the branch that triggered the workflow as its base branch. Use `cwd`, `base-branch`, `branch`, `check`, `version`, `commit-message`, and `pr-title` to adapt it to a different repository layout or release policy. The pull request body is always generated from the changelog diff and does not need to be configured.

## Releasing the Action

The action is consumed directly from this repository; Marketplace publication is optional. Run the `Release GitHub Action` workflow manually with a semantic version such as `1.0.0`. It creates:

- an immutable release tag such as `v1.0.0`
- a GitHub release with generated notes
- the moving major compatibility tag `v1`

Consumers can use the major tag:

```yaml
- uses: bdbch/rivet@v1
```

For maximum supply-chain pinning, use the commit SHA from the action release instead.
