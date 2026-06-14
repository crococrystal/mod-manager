#!/usr/bin/env bash
set -euo pipefail

# Compare local mod-manager view vs remote SSH for Crystal Tech sync.
# Usage: bash scripts/compare-server-sync.sh

SSH_HOST="${SSH_HOST:-win-test}"
INSTANCE="${INSTANCE_ROOT:-/Users/crococrystal/Library/Application Support/PrismLauncher/instances/Crystal Tech 1.21.1}"
REMOTE_SERVER="${REMOTE_SERVER:-C:/Users/Admin/Desktop/Crystal Tech 1.21.1/mods}"
REMOTE_DIST="${REMOTE_DIST:-C:/Users/Admin/Desktop/Crystal Tech 1.21.1/automodpack/host-modpack/main/mods}"

PATH1="$INSTANCE/minecraft/mods"
MINECRAFT_DIR="$INSTANCE/minecraft"
PATH2=""

if [[ -f "$MINECRAFT_DIR/automodpack/automodpack-client.json" ]]; then
  SELECTED=$(python3 - <<PY
import json, pathlib
p = pathlib.Path("$MINECRAFT_DIR/automodpack/automodpack-client.json")
try:
    print(json.loads(p.read_text()).get("selectedModpack", "").strip())
except Exception:
    print("")
PY
)
  if [[ -n "$SELECTED" && -d "$MINECRAFT_DIR/automodpack/modpacks/$SELECTED/mods" ]]; then
    PATH2="$MINECRAFT_DIR/automodpack/modpacks/$SELECTED/mods"
  fi
fi

if [[ -z "$PATH2" ]]; then
  for d in "$MINECRAFT_DIR/automodpack/modpacks"/*/mods; do
    [[ -d "$d" ]] || continue
    PATH2="$d"
    break
  done
fi

remote_list() {
  local remote_dir="$1"
  local ps_path="${remote_dir//\//\\}"
  ssh "$SSH_HOST" "powershell -NoProfile -Command \"Get-ChildItem -LiteralPath '$ps_path' -Filter *.jar -File -ErrorAction SilentlyContinue | ForEach-Object { Write-Output (\$_.Name + '|' + \$_.Length) }\"" \
    | sort
}

local_list() {
  local dir="$1"
  [[ -d "$dir" ]] || return 0
  stat -f '%z %N' "$dir"/*.jar 2>/dev/null | while read -r size path; do
    basename "$path" | awk -v s="$size" '{print $0 "|" s}'
  done | sort
}

compare_lane() {
  local label="$1"
  local remote_dir="$2"
  local tmp
  tmp=$(mktemp -d)

  local_list "$PATH1" > "$tmp/p1.txt"
  if [[ -n "$PATH2" && -d "$PATH2" ]]; then
    local_list "$PATH2" > "$tmp/p2.txt"
  else
    : > "$tmp/p2.txt"
  fi
  remote_list "$remote_dir" > "$tmp/remote.txt" || true

  python3 - "$label" "$PATH1" "$PATH2" "$remote_dir" "$tmp/p1.txt" "$tmp/p2.txt" "$tmp/remote.txt" <<'PY'
import sys

label, path1, path2, remote_dir, p1f, p2f, rf = sys.argv[1:8]

def load(path):
    m = {}
    try:
        for line in open(path):
            line = line.strip()
            if not line or "|" not in line:
                continue
            name, size = line.rsplit("|", 1)
            m[name] = int(size)
    except FileNotFoundError:
        pass
    return m

p1, p2, remote = load(p1f), load(p2f), load(rf)
union = {**p2, **p1}
dup = sorted(set(p1) & set(p2))
dup_same = [n for n in dup if p1[n] == p2[n]]
dup_diff = [n for n in dup if p1[n] != p2[n]]
local_only = sorted(set(union) - set(remote))
remote_only = sorted(set(remote) - set(union))
size_mismatch = sorted(n for n in set(union) & set(remote) if union[n] != remote[n])
synced = sorted(n for n in set(union) & set(remote) if union[n] == remote[n])

print(f"\n{'=' * 72}")
print(f"LANE: {label}")
print(f"Local PATH1: {path1}")
print(f"Local PATH2: {path2 or '(none)'}")
print(f"Remote:      {remote_dir}")
print(f"Local union: {len(union)}  Remote: {len(remote)}  Synced: {len(synced)}")
print(f"TO UPLOAD:   {len(local_only)}  TO DELETE: {len(remote_only)}  SIZE MISMATCH: {len(size_mismatch)}")
if dup:
    print(f"Duplicates in both local dirs: {len(dup)} (same size: {len(dup_same)}, diff size: {len(dup_diff)})")
    for n in dup_diff[:10]:
        print(f"  DIFF {n}: path1={p1[n]} path2={p2[n]}")
if local_only:
    print("\n--- UPLOAD (local only) ---")
    for n in local_only:
        print(f"  {n}\t{union[n]}")
if remote_only:
    print("\n--- DELETE on remote (remote only) ---")
    for n in remote_only:
        print(f"  {n}\t{remote[n]}")
if size_mismatch:
    print("\n--- SIZE MISMATCH (re-upload) ---")
    for n in size_mismatch:
        print(f"  {n}\tlocal={union[n]}\tremote={remote[n]}")
PY

  rm -rf "$tmp"
}

echo "Instance: $INSTANCE"
echo "SSH: $SSH_HOST"
compare_lane "SERVER (non-client mods only — filter manually if needed)" "$REMOTE_SERVER"
compare_lane "DISTRIBUTION (all mods)" "$REMOTE_DIST"
