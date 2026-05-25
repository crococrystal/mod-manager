#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Ключи из .env (тот же TAURI_SIGNING_PRIVATE_KEY, что в GitHub Secrets)
set -a && [ -f .env ] && . ./.env && set +a
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"

RELEASES_REPO="${RELEASES_REPO:-crococrystal/mod-manager-releases}"
BUNDLE="$ROOT/src-tauri/target/release/bundle"
STAMP="$ROOT/node_modules/.package-lock.sha"
STAGING=""

cleanup() { [ -n "$STAGING" ] && rm -rf "$STAGING"; }
trap cleanup EXIT

ensure_npm_deps() {
  if [ "${SKIP_NPM_CI:-0}" = "1" ] && [ -d node_modules ]; then return; fi
  local hash
  hash="$(shasum -a 256 package-lock.json | awk '{print $1}')"
  if [ -d node_modules ] && [ -f "$STAMP" ] && [ "$(cat "$STAMP")" = "$hash" ]; then return; fi
  echo "→ npm ci…"
  npm ci
  echo "$hash" >"$STAMP"
}

if [ "${SKIP_BUILD:-0}" != "1" ]; then
  ensure_npm_deps
  echo "→ tauri build…"
  npx tauri build --bundles app
fi

STAGING="$(mktemp -d)"
mkdir -p "$STAGING/out"
find "$BUNDLE" -type f \( -name '*.app.tar.gz' -o -name '*.app.tar.gz.sig' \) -exec cp {} "$STAGING/out/" \;

if ! compgen -G "$STAGING/out/*.app.tar.gz.sig" >/dev/null; then
  echo "Нет подписи — проверь TAURI_SIGNING_PRIVATE_KEY в .env" >&2
  exit 1
fi

./scripts/generate-latest-json.sh "$STAGING/out" "$STAGING/out/latest.json"

echo "→ upload…"
if gh release view latest --repo "$RELEASES_REPO" >/dev/null 2>&1; then
  gh release view latest --repo "$RELEASES_REPO" --json assets --jq '.assets[].name' | while read -r asset; do
    [ -z "$asset" ] && continue
    gh release delete-asset latest "$asset" --repo "$RELEASES_REPO" -y
  done
else
  gh release create latest --repo "$RELEASES_REPO" --title "Latest build" \
    --notes "Local $(git -C "$ROOT" rev-parse --short HEAD)" --latest --target main
fi

gh release upload latest "$STAGING/out"/* --repo "$RELEASES_REPO" --clobber
echo "https://github.com/${RELEASES_REPO}/releases/latest"
