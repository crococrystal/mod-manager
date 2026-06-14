#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

set -a
[ -f .env ] && . ./.env
set +a

if [ "${PREVIEW_WINDOWS_CHROME:-}" = "1" ] || [ "${VITE_PREVIEW_WINDOWS_CHROME:-}" = "true" ] || [ "${VITE_PREVIEW_WINDOWS_CHROME:-}" = "1" ]; then
  export PREVIEW_WINDOWS_CHROME=1
  export VITE_PREVIEW_WINDOWS_CHROME=true
fi

# Keep Rust artifacts under this checkout (avoids stale permission files from another path).
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/src-tauri/target}"

exec npx tauri dev "$@"
