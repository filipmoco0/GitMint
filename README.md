<div align="center">
  <img src="public/brand/wordmark.svg" width="300" alt="GitMint — Your Git workflow, refreshed." />

  <p>A focused, open-source desktop Git client. Browse your repositories, review changes, and make everyday Git work feel clear.</p>

  <p><a href="https://github.com/filipmoco0/GitMint/releases/latest"><strong>Download GitMint</strong></a> &nbsp;·&nbsp; <a href="https://github.com/filipmoco0/GitMint/issues">Report a problem</a> &nbsp;·&nbsp; <a href="CONTRIBUTING.md">Contribute</a></p>

  <p><img alt="Rust" src="https://img.shields.io/badge/Rust-app_logic-7A3E20?logo=rust&logoColor=white" /> <img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white" /> <img alt="macOS" src="https://img.shields.io/badge/macOS-Apple_silicon-111111?logo=apple&logoColor=white" /> <img alt="MIT License" src="https://img.shields.io/github/license/filipmoco0/GitMint" /> <img alt="Status" src="https://img.shields.io/badge/status-public_release-A6D6B0" /></p>

  <img src="docs/images/gitmint-preview.png" alt="GitMint repository library on macOS" width="100%" />
</div>

## A calmer way to work with Git

GitMint brings the everyday parts of Git into one compact desktop app:

- Browse GitHub repositories and clone one to your Mac.
- Open local repositories and return to recent projects.
- Read commit history and inspect the files in each commit.
- Review staged and unstaged changes with readable diffs.
- Stage files, create commits, manage local branches, and sync with GitHub.
- Keep branch switches safe: GitMint stops when local changes could be affected.

Git operations run locally through Git. GitHub sign-in authenticates repository browsing, cloning, pushing, and pulling. Pull uses fast-forward only and stops when the working tree has changes. GitHub HTTPS remotes are supported for sync; pull requests and merge workflows are not included.

## Get GitMint

Download available builds from [GitHub Releases](https://github.com/filipmoco0/GitMint/releases/latest). Current published releases target Apple silicon. Windows and Linux installers will be produced by the desktop build workflow; publish the generated artifacts to a GitHub Release to make them downloadable. Please keep a backup of important work and report issues on [GitHub](https://github.com/filipmoco0/GitMint/issues).

## Build from source

Requirements: Git, Node.js, and Rust. Platform-specific prerequisites are listed in the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/).

1. Copy `.env.example` to `.env.local`. It includes the public GitMint OAuth client ID and HTTPS token-broker URL. For a custom OAuth app or broker, see [AUTHENTICATION.md](AUTHENTICATION.md).
2. Install dependencies and start the app:

```sh
npm install
npm run tauri:dev
```

3. Build for your current desktop platform:

```sh
npm run tauri:build
```

On macOS this builds the `.app` bundle. To create a DMG, use `npm run package:macos-dmg`. On Windows, Tauri creates NSIS and MSI installers; on Linux, it creates Debian, RPM, and AppImage packages. Each platform must build its own native installers. macOS requires Xcode Command Line Tools, Windows requires Microsoft C++ Build Tools and WebView2, and Linux requires the Tauri system libraries.

Linux GitHub sign-in uses the desktop Secret Service keyring (for example, GNOME Keyring or KWallet). Install and unlock a compatible keyring in your desktop session before signing in.

## Privacy

GitMint stores your GitHub sign-in token in the operating system credential store: Keychain on macOS, Windows Credential Manager on Windows, and Secret Service on Linux. The token broker exchanges OAuth authorization codes and does not store tokens. Git operations are scoped to the selected local repository. Read [AUTHENTICATION.md](AUTHENTICATION.md) for details about the sign-in flow and permissions.

## Community

- [Contributing guide](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [Open an issue](https://github.com/filipmoco0/GitMint/issues)
- [MIT License](LICENSE)
