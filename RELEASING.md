# Releasing

Releases are intentionally promoted, rather than published from every merge to `main`.

## Flow

1. Add Rivet release files with `rivet new` in feature branches.
2. Run `rivet bump` on a release-preparation branch. Review the version changes, changelogs, and `.rivet/releaseplan.txt`.
3. Open and merge that bump PR into `main`.
4. Promote the merged commit to the protected `release` branch, preferably with a pull request from `main` to `release`.
5. The `Release` workflow validates and publishes the packages listed in `.rivet/releaseplan.txt` using their committed versions.

For a direct promotion, fast-forward the branch locally and push it:

```bash
git fetch origin
git switch release
git merge --ff-only origin/main
git push origin release
```

The `release` branch is an explicit production gate. It must not be a wildcard such as `release/*`, because every push to it can publish to npm.

## GitHub setup

- Protect the `release` branch and require the release workflow to pass.
- Create a `npm-production` environment with required reviewers.
- Add `NPM_TOKEN` as a secret on that environment.
- Configure the package version and changelog in the bump PR before promoting it.

The release workflow uses `rivet release`, which reads `.rivet/releaseplan.txt`, publishes the committed package versions, skips versions already present on npm, and removes the plan in the CI workspace after a successful release.
