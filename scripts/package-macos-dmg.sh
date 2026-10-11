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

# Tauri's linker signature is not a complete bundle signature. Build-host
# extended attributes can also make codesign reject the bundle as damaged.
# Strip that metadata, ad-hoc sign the complete bundle, and verify the seal
# before placing it in the downloadable DMG. A Developer ID is still needed
# for notarization and a warning-free public macOS install.
xattr -cr "$APP_PATH"
# Finder can reattach this bundle-level attribute between builds, and codesign
# rejects it even after the recursive cleanup above.
xattr -d com.apple.FinderInfo "$APP_PATH" 2>/dev/null || true
BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP_PATH/Contents/Info.plist")"
codesign --force --deep --sign - --identifier "$BUNDLE_ID" "$APP_PATH"
codesign --verify --deep --strict --verbose=2 "$APP_PATH"

"$VENV_DIR/bin/dmgbuild" \
  -s "$PROJECT_ROOT/src-tauri/installer/dmgbuild_settings.py" \
  -D "app=$APP_PATH" \
  -D "background=$PROJECT_ROOT/src-tauri/installer/dmg-background.png" \
  -D "volume_icon=$PROJECT_ROOT/src-tauri/icons/icon.icns" \
  "GitMint Installer" "$OUTPUT_PATH"

# dmgbuild stores Finder positioning metadata on the app bundle. Remove it and
# reseal the app, then replace the DMG copy so Finder metadata cannot break the
# signature after packaging.
xattr -d com.apple.FinderInfo "$APP_PATH" 2>/dev/null || true
codesign --force --deep --sign - --identifier "$BUNDLE_ID" "$APP_PATH"
codesign --verify --deep --strict --verbose=2 "$APP_PATH"

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gitmint-dmg.XXXXXX")"
WORKING_DMG="$WORK_DIR/GitMint-writable.dmg"
SIGNED_DMG="$WORK_DIR/GitMint-signed.dmg"
MOUNT_DIR="$WORK_DIR/mount"
mkdir -p "$MOUNT_DIR"
hdiutil convert "$OUTPUT_PATH" -format UDRW -o "$WORKING_DMG"
hdiutil attach -nobrowse -readwrite -mountpoint "$MOUNT_DIR" "$WORKING_DMG"
IMAGE_APP="$MOUNT_DIR/GitMint.app"
rm -rf "$IMAGE_APP"
ditto "$APP_PATH" "$IMAGE_APP"
xattr -d com.apple.FinderInfo "$IMAGE_APP" 2>/dev/null || true
codesign --verify --deep --strict --verbose=2 "$IMAGE_APP"
hdiutil detach "$MOUNT_DIR"
hdiutil convert "$WORKING_DMG" -format UDZO -o "$SIGNED_DMG"
mv -f "$SIGNED_DMG" "$OUTPUT_PATH"
rm -rf "$WORK_DIR"
hdiutil verify "$OUTPUT_PATH"

echo "Created $OUTPUT_PATH"
