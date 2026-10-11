# GitMint

**Your Git workflow, refreshed.**

GitMint is a lightweight, open-source Git client for macOS. Browse GitHub repositories, open local projects, and handle everyday Git work in a focused desktop app.

> **Public beta:** GitMint is early software. Back up important work and report problems through GitHub Issues once the public repository is available.

## What works

- Sign in with GitHub and browse repositories you can access.
- Clone a GitHub repository to this Mac and open it in GitMint.
- Open existing local Git repositories and switch between recent ones.
- Browse commit history and inspect changed files.
- Review staged and unstaged changes, inspect diffs, stage files, and create commits.
- List, create, and switch local branches. Switching is blocked when local changes could be affected.

GitMint currently does **not** provide push, pull, pull request, or merge workflows. GitHub is used for sign-in and repository browsing; local Git operations use the Git CLI.

## Download

The first beta is being prepared for GitHub Releases. For now, build the macOS installer from source using the steps below.

## Build from source

Requirements: macOS, Git, Node.js, Rust, and Xcode Command Line Tools.

1. Copy `.env.example` to `.env.local`. It includes the public beta OAuth client ID and HTTPS token-broker URL. For a custom OAuth app or broker, follow [AUTHENTICATION.md](AUTHENTICATION.md).
2. Install dependencies and run the app:

   ```sh
   npm install
   npm run tauri:dev
   ```

3. Build a branded Apple silicon installer:

   ```sh
   npm run package:macos-dmg
   ```

The installer is written to `src-tauri/target/release/bundle/dmg/`. `npm run tauri:build` builds the `.app` bundle only.

## Checks

```sh
npm run build
npm run lint
cd src-tauri && cargo check
```

## Privacy and security

GitMint stores the GitHub sign-in token in macOS Keychain. The token broker exchanges OAuth authorization codes and does not store tokens. Git operations run against the selected local repository. GitHub OAuth's `repo` permission is broader than read-only; the app currently uses GitHub to list and open repositories. Read [AUTHENTICATION.md](AUTHENTICATION.md) before building or deploying the sign-in flow.

## License

GitMint is licensed under the MIT License. See [LICENSE](LICENSE).
