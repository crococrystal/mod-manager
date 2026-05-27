#!/usr/bin/env bash
set -euo pipefail

artifacts_dir="${1:-artifacts}"
out_file="${2:-artifacts/latest.json}"
version="$(node -p "require('./package.json').version")"
base_url="${RELEASE_BASE:-https://github.com/crococrystal/mod-manager/releases/download/latest}"
notes="${UPDATE_NOTES:-Latest build}"
pub_date="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

read_signature() {
  tr -d '\n' <"$1"
}

# GitHub Releases stores assets with spaces replaced by dots in the filename.
github_release_asset_name() {
  printf '%s' "$1" | tr ' ' '.'
}

platforms='{}'

mac_bundle="$(find "$artifacts_dir" -type f -name '*.app.tar.gz' ! -name '*.sig' 2>/dev/null | head -n 1 || true)"
if [ -n "$mac_bundle" ] && [ -f "${mac_bundle}.sig" ]; then
  mac_name="$(basename "$mac_bundle")"
  mac_sig="$(read_signature "${mac_bundle}.sig")"
  mac_url="${base_url}/$(github_release_asset_name "$mac_name")"
  platforms="$(jq -n \
    --argjson current "$platforms" \
    --arg url "$mac_url" \
    --arg signature "$mac_sig" \
    '$current + {"darwin-aarch64": {url: $url, signature: $signature}}')"
fi

win_bundle="$(find "$artifacts_dir" -type f -name '*-setup.exe' ! -name '*.sig' 2>/dev/null | head -n 1 || true)"
if [ -n "$win_bundle" ] && [ -f "${win_bundle}.sig" ]; then
  win_name="$(basename "$win_bundle")"
  win_sig="$(read_signature "${win_bundle}.sig")"
  win_url="${base_url}/$(github_release_asset_name "$win_name")"
  platforms="$(jq -n \
    --argjson current "$platforms" \
    --arg url "$win_url" \
    --arg signature "$win_sig" \
    '$current + {"windows-x86_64": {url: $url, signature: $signature}}')"
fi

linux_bundle="$(find "$artifacts_dir" -type f -name '*.AppImage' ! -name '*.sig' 2>/dev/null | head -n 1 || true)"
if [ -n "$linux_bundle" ] && [ -f "${linux_bundle}.sig" ]; then
  linux_name="$(basename "$linux_bundle")"
  linux_sig="$(read_signature "${linux_bundle}.sig")"
  linux_url="${base_url}/$(github_release_asset_name "$linux_name")"
  platforms="$(jq -n \
    --argjson current "$platforms" \
    --arg url "$linux_url" \
    --arg signature "$linux_sig" \
    '$current + {"linux-x86_64": {url: $url, signature: $signature}}')"
fi

if [ "$platforms" = '{}' ]; then
  echo "No signed updater bundles found in ${artifacts_dir}" >&2
  exit 1
fi

jq -n \
  --arg version "$version" \
  --arg notes "$notes" \
  --arg pub_date "$pub_date" \
  --argjson platforms "$platforms" \
  '{version: $version, notes: $notes, pub_date: $pub_date, platforms: $platforms}' >"$out_file"

echo "Wrote ${out_file}"
