#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUNDLE_DIR=""
for candidate in \
  "$ROOT/src-tauri/target/cargo-target/release/bundle" \
  "$ROOT/src-tauri/target/release/bundle"; do
  if [[ -d "$candidate/macos/gpt2cursor.app" ]]; then
    BUNDLE_DIR="$candidate"
    break
  fi
done

if [[ -z "$BUNDLE_DIR" ]]; then
  echo "Missing app bundle under src-tauri/target/*/release/bundle/macos/" >&2
  echo "Run npm run tauri build first." >&2
  exit 1
fi

APP_PATH="$BUNDLE_DIR/macos/gpt2cursor.app"
DMG_TOOL_DIR="$BUNDLE_DIR/dmg"
VERSION="$(node -p "require('$ROOT/package.json').version")"
DMG_PATH="$DMG_TOOL_DIR/gpt2cursor_${VERSION}_aarch64.dmg"
BACKGROUND="$ROOT/src-tauri/dmg/background.png"
VOLICON="$DMG_TOOL_DIR/gpt2cursor.icns"

if [[ ! -f "$DMG_TOOL_DIR/bundle_dmg.sh" ]]; then
  echo "Missing bundle_dmg.sh in $DMG_TOOL_DIR" >&2
  echo "Run npm run tauri build first." >&2
  exit 1
fi

echo "Signing gpt2cursor.app for macOS distribution..."

# Linker adhoc signatures set CSResourcesFileMapped without sealing Resources,
# which makes Gatekeeper report the app as damaged on double-click.
/usr/libexec/PlistBuddy -c "Delete :CSResourcesFileMapped" "$APP_PATH/Contents/Info.plist" 2>/dev/null || true

xattr -cr "$APP_PATH"

BIN="$APP_PATH/Contents/MacOS/gpt2cursor"
codesign --force --sign - "$BIN"
codesign --force --sign - "$APP_PATH"

echo "Verifying signature..."
codesign --verify --deep --strict --verbose=2 "$APP_PATH"

STAGING_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gpt2cursor-dmg.XXXXXX")"
cleanup() {
  rm -rf "$STAGING_DIR"
}
trap cleanup EXIT

ditto "$APP_PATH" "$STAGING_DIR/gpt2cursor.app"
rm -f "$STAGING_DIR/.DS_Store"

BUNDLE_DMG_ARGS=(
  --volname "gpt2cursor"
  --window-size 660 400
  --icon "gpt2cursor.app" 180 220
  --hide-extension "gpt2cursor.app"
  --app-drop-link 480 220
)

if [[ -f "$VOLICON" ]]; then
  BUNDLE_DMG_ARGS+=(--volicon "$VOLICON")
fi

if [[ -f "$BACKGROUND" ]]; then
  BUNDLE_DMG_ARGS+=(--background "$BACKGROUND")
fi

echo "Building installer DMG with Applications shortcut..."
rm -f "$DMG_PATH" "$DMG_TOOL_DIR/rw.$(basename "$DMG_PATH")"
bash "$DMG_TOOL_DIR/bundle_dmg.sh" "${BUNDLE_DMG_ARGS[@]}" "$DMG_PATH" "$STAGING_DIR"

echo "Rebuilt DMG: $DMG_PATH"
spctl -a -vv "$APP_PATH" 2>&1 || true
