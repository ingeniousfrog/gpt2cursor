#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUNDLE_DIR="$ROOT/src-tauri/target/release/bundle"
APP_PATH="$BUNDLE_DIR/macos/gpt2cursor.app"
VERSION="$(node -p "require('$ROOT/package.json').version")"
DMG_PATH="$BUNDLE_DIR/dmg/gpt2cursor_${VERSION}_aarch64.dmg"

if [[ ! -d "$APP_PATH" ]]; then
  echo "Missing app bundle: $APP_PATH" >&2
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

mkdir -p "$BUNDLE_DIR/dmg"
rm -f "$DMG_PATH"
hdiutil create \
  -volname "gpt2cursor" \
  -srcfolder "$APP_PATH" \
  -ov \
  -format UDZO \
  "$DMG_PATH" >/dev/null

echo "Rebuilt DMG: $DMG_PATH"
spctl -a -vv "$APP_PATH" 2>&1 || true
