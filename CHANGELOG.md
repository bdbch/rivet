# Changelog

## @bdbchgg/rivet (v1.0.0-alpha.6)

### Patch Changes

- Fix a bug where the changelog format for solorepo workspaces would not use the normal mono-repo format
- Add guard to `rivet init` to detect already-initialized projects and exit early with an informational message. Use `--force` to re-run the wizard and overwrite the existing config.
