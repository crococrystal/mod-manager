#!/usr/bin/env bash
set -euo pipefail

releases_page="https://github.com/${RELEASES_REPO:-crococrystal/mod-manager-releases}/releases/latest"

cat >"${1:-README.releases.md}" <<EOF
# Mod Manager

Desktop app for Minecraft modpacks: mod tags, dependencies, version and provider switching (Modrinth / CurseForge).

**Download:** ${releases_page}
EOF

echo "Wrote ${1:-README.releases.md}"
