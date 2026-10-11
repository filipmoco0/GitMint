# Contributing to GitMint

Thanks for helping make everyday Git work clearer. Focused bug fixes, usability improvements, and small, well-scoped features are especially helpful.

## Before you start

- Search [existing issues](https://github.com/filipmoco0/GitMint/issues) before opening a new one.
- For a larger change, open an issue first to discuss its scope.
- Please do not include access tokens, private repository contents, or personal data in an issue or screenshot.

## Work locally

GitMint uses Rust, Tauri 2, React, TypeScript, and Vite. You will need macOS, Git, Node.js, Rust, and Xcode Command Line Tools.

```sh
npm install
cp .env.example .env.local
npm run tauri:dev
```

The checked-in example contains the public sign-in configuration. Keep custom credentials and secrets in `.env.local`; never commit them.

Before opening a pull request, run the checks relevant to your change:

```sh
npm run build
npm run lint
cd src-tauri && cargo check
```

## Project principles

- Keep the app focused on everyday repository work and macOS quality.
- Run Git with process arguments, scoped to the selected repository.
- Preserve user changes; never reset or force-switch branches silently.
- Prefer simple, typed Tauri commands and reusable UI components.
- Avoid adding analytics, cloud services, or dependencies without a clear need.

## Pull requests

Describe the user-facing change, note any safety impact, and include screenshots for visible UI changes when practical. Keep each pull request focused and mention which checks you ran.

By submitting a contribution, you agree that it may be distributed under the project's [MIT License](LICENSE).
