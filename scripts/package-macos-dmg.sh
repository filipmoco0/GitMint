#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VENV_DIR="${TMPDIR:-/tmp}/gitmint-dmgbuild-venv"
APP_PATH="$PROJECT_ROOT/src-tauri/target/release/bundle/macos/GitMint.app"
OUTPUT_DIR="$PROJECT_ROOT/src-tauri/target/release/bundle/dmg"
VERSION="$(node -p "require('$PROJECT_ROOT/package.json').version")"
OUTPUT_PATH="$OUTPUT_DIR/GitMint_${VERSION}_aarch64.dmg"

if [[ ! -x "$VENV_DIR/bin/dmgbuild" ]]; then
  python3 -m venv "$VENV_DIR"
  "$VENV_DIR/bin/python" -m pip install --quiet --upgrade dmgbuild
fi

# Tauri assembles the native .app; dmgbuild writes Finder's DMG layout metadata
# directly, avoiding the background-picture AppleScript that macOS Tahoe ignores.
cd "$PROJECT_ROOT"
npm run tauri:build
mkdir -p "$OUTPUT_DIR"
"$VENV_DIR/bin/dmgbuild" \
  -s "$PROJECT_ROOT/src-tauri/installer/dmgbuild_settings.py" \
  -D "app=$APP_PATH" \
  -D "background=$PROJECT_ROOT/src-tauri/installer/dmg-background.png" \
  -D "volume_icon=$PROJECT_ROOT/src-tauri/icons/icon.icns" \
  "GitMint Installer" "$OUTPUT_PATH"

echo "Created $OUTPUT_PATH"
