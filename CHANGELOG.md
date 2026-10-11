# Changelog

## 1.0.0

First public release.

- Browse and clone GitHub repositories after signing in.
- Open and switch between local repositories.
- Inspect commit history, changed files, and diffs.
- Stage and unstage changes, stage all, and create commits.
- Create and switch local branches with protection for uncommitted changes.
- Push branches to GitHub and pull fast-forward updates from GitHub.
- Choose whether the repository library opens to GitHub or local repositories.
- Use the custom GitMint macOS installer artwork.

### Known limitations

- Pull requests and merge workflows are not included yet.
- The first installer targets Apple silicon Macs.
- GitHub sync currently requires HTTPS remotes. Pull only accepts fast-forward updates and stops when local changes exist.
- Builds are not signed with an Apple Developer ID certificate or notarized. macOS may block opening the app until users approve it in Privacy & Security settings.
