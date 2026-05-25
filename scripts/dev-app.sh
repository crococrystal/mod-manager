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

exec npx tauri dev "$@"
