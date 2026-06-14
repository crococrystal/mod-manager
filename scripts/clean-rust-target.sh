#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
rm -rf src-tauri/target
echo "Removed src-tauri/target — run npm run dev:app:win again (first build will take a few minutes)."
