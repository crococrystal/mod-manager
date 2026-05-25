#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_PATH="$ROOT/src-tauri/target/release/bundle/macos/mod-manager.app"
OUT_DIR="$ROOT/src-tauri/target/release/bundle/dmg"
STAGE="$OUT_DIR/stage"
DMG_PATH="$OUT_DIR/mod-manager_0.1.0_aarch64.dmg"

if [[ ! -d "$APP_PATH" ]]; then
  echo "Missing app bundle: $APP_PATH" >&2
  exit 1
fi

rm -rf "$STAGE"
mkdir -p "$STAGE" "$OUT_DIR"
cp -R "$APP_PATH" "$STAGE/"
ln -s /Applications "$STAGE/Applications"

hdiutil create \
  -volname "mod-manager" \
  -srcfolder "$STAGE" \
  -ov \
  -format UDZO \
  "$DMG_PATH"

rm -rf "$STAGE"
echo "$DMG_PATH"
