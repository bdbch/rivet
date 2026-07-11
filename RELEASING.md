# Releasing

Releases are intentionally prepared in a pull request, rather than published from every feature branch.

## Flow

1. Add Rivet release files with `rivet new` in feature branches.
2. Merge those changes into `main`.
3. The `Release` workflow runs `rivet bump` on the `rivet-release` branch and creates or updates a version pull request.
4. Review and merge the version pull request, including versions, changelogs, and `.rivet/releaseplan.txt`.
5. The next `main` push runs the configured native build and `rivet release` for the committed versions.

## GitHub setup

- Add `NPM_TOKEN` as a repository secret. The workflow also needs `GITHUB_TOKEN` permissions for `contents: write` and `pull-requests: write`.
- Configure the package version and changelog in the bump PR before promoting it.

The release workflow uses the local Rivet GitHub Action. It reads `.rivet/releaseplan.txt`, publishes the committed package versions, skips versions already present on npm, and removes the plan in the CI workspace after a successful release.
