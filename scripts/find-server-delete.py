#!/usr/bin/env python3
"""Find server-sync orphans and classify UPDATE vs DELETE (matches server_sync.rs)."""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

SSH_HOST = "win-test"
INSTANCE = Path(
    os.environ.get(
        "INSTANCE_ROOT",
        "/Users/crococrystal/Minecraft/Crystal Tech",
    )
)
REMOTE_SERVER = "C:/Users/Admin/Desktop/Crystal Tech 1.21.1/mods"


# --- mod_names.rs ---


def is_emoji_char(ch: str) -> bool:
    cp = ord(ch)
    return (
        0x1F300 <= cp <= 0x1FAFF
        or 0x2600 <= cp <= 0x27BF
        or 0x1F600 <= cp <= 0x1F64F
        or 0x1F900 <= cp <= 0x1F9FF
        or 0x1F1E6 <= cp <= 0x1F1FF
        or cp in (0x200D, 0xFE0F)
    )


def strip_filename_decorations(value: str) -> str:
    value = value.strip()
    while value and (
        value[0].isspace()
        or value[0] in "-_+()[]"
        or is_emoji_char(value[0])
    ):
        value = value[1:]
    return value.strip()


def strip_qualifiers(value: str) -> str:
    result: list[str] = []
    depth = 0
    for ch in value:
        if ch in "([":
            depth += 1
        elif ch in ")]":
            depth = max(0, depth - 1)
        elif depth == 0:
            result.append(ch)
    return "".join(result).strip()


def normalized_match_key(value: str) -> str:
    return "".join(ch for ch in value.lower() if ch.isascii() and ch.isalnum())


def is_version_or_loader_segment(segment: str) -> bool:
    s = segment.strip().lower()
    if not s:
        return True
    if s in {
        "neoforge",
        "forge",
        "fabric",
        "quilt",
        "client",
        "server",
        "universal",
        "both",
    }:
        return True
    if s.startswith("mc") and len(s) > 2 and all(ch.isdigit() or ch == "." for ch in s[2:]):
        return True
    if s.startswith("v") and len(s) > 1 and s[1].isdigit():
        return True
    needles = (
        "hotfix",
        "hotfix2",
        "beta",
        "alpha",
        "rc",
        "pre",
        "release",
        "snapshot",
        "snap",
        "patch",
    )
    if any(needle in s for needle in needles):
        return True
    if (
        len(s) <= 16
        and any(ch.isdigit() for ch in s)
        and any(ch.isalpha() for ch in s)
        and sum(1 for ch in s if ch.isdigit()) <= 4
    ):
        return True
    has_digit = False
    for ch in s:
        if ch.isdigit():
            has_digit = True
        elif ch not in ".-+barc":
            return False
    if has_digit and "." in s:
        return True
    if "+" in s:
        parts = s.split("+")
        return all(
            not part.strip()
            or (
                any(ch.isdigit() for ch in part)
                and all(ch.isdigit() or ch in ".-+" for ch in part)
            )
            for part in parts
        )
    return False


def mod_name_tokens(value: str) -> list[str]:
    clean = strip_qualifiers(strip_filename_decorations(value))
    parts = [part for part in re.split(r"[-_]", clean) if part]
    while len(parts) > 1 and is_version_or_loader_segment(parts[-1]):
        parts.pop()
    return parts


def strip_version_suffixes(value: str) -> str:
    tokens = mod_name_tokens(value)
    if not tokens:
        return strip_qualifiers(strip_filename_decorations(value))
    if len(tokens) == 1:
        return tokens[0]
    return "-".join(tokens)


def mod_sync_identity_key(filename: str) -> str:
    stem = filename.removesuffix(".jar")
    clean = strip_filename_decorations(stem)
    return normalized_match_key(strip_version_suffixes(clean))


# --- provider_labels.rs / mods.rs ---


def normalize_side(side: str) -> str:
    side = side.strip().lower()
    if side == "client":
        return "client"
    if side == "server":
        return "server"
    return "universal"


def side_mode_for(tag: dict) -> str:
    mode = tag.get("labelOverrides", {}).get("sideMode", "").strip()
    return "manual" if mode == "manual" else "auto"


def stored_side(tag: dict) -> str:
    side = tag.get("side", "").strip()
    return normalize_side(side or "universal")


def map_provider_side(store: dict) -> str | None:
    if not store.get("fetchedAt", "").strip():
        return None
    client = store.get("clientSide", "")
    server = store.get("serverSide", "")
    side_map = {
        ("required", "unsupported"): "client",
        ("unsupported", "required"): "server",
        ("required", "required"): "universal",
        ("optional", "optional"): "universal",
        ("required", "optional"): "universal",
        ("optional", "required"): "universal",
    }
    return normalize_side(side_map.get((client, server), "universal"))


def resolve_side(tag: dict) -> str:
    if side_mode_for(tag) == "manual":
        return stored_side(tag)
    fetched = tag.get("providerLabels", {}).get("fetchedAt", "").strip()
    if not fetched:
        return normalize_side("universal")
    return map_provider_side(tag.get("providerLabels", {})) or normalize_side("universal")


def slug_from_filename(filename: str) -> str:
    base = filename.removesuffix(".jar")
    result: list[str] = []
    previous_dash = False
    for ch in base:
        if ch.isalnum():
            result.append(ch.lower())
            previous_dash = False
        elif not previous_dash:
            result.append("-")
            previous_dash = True
    return "".join(result).strip("-")


@dataclass
class IndexInfo:
    modrinth_id: str | None = None
    curseforge_id: str | None = None


@dataclass
class ModEntry:
    key: str
    filename: str
    side: str
    local_path: Path
    local_size: int


def read_index(index_dir: Path) -> dict[str, IndexInfo]:
    out: dict[str, IndexInfo] = {}
    if not index_dir.is_dir():
        return out
    for path in index_dir.glob("*.pw.toml"):
        try:
            import tomllib
        except ModuleNotFoundError:
            import tomli as tomllib  # type: ignore
        try:
            data = tomllib.loads(path.read_text(encoding="utf-8"))
        except Exception:
            continue
        filename = data.get("filename")
        if not isinstance(filename, str) or not filename:
            continue
        update = data.get("update", {})
        modrinth = update.get("modrinth", {}) if isinstance(update, dict) else {}
        curseforge = update.get("curseforge", {}) if isinstance(update, dict) else {}
        out[filename] = IndexInfo(
            modrinth_id=str(modrinth.get("mod-id", "")).strip() or None,
            curseforge_id=str(curseforge.get("project-id", "")).strip() or None,
        )
    return out


def alias_keys_by_filename(tags: dict) -> dict[str, str]:
    entries = sorted(tags.get("mods", {}).items(), key=lambda item: item[0])
    out: dict[str, str] = {}
    for key, tag in entries:
        for alias in tag.get("aliases", []):
            alias = alias.strip()
            if alias and alias not in out:
                out[alias] = key
    return out


def stable_key(filename: str, info: IndexInfo | None) -> str:
    if info and info.modrinth_id:
        return f"modrinth:{info.modrinth_id}"
    if info and info.curseforge_id:
        return f"curseforge:{info.curseforge_id}"
    return f"manual:{slug_from_filename(filename)}"


def key_for_file(
    filename: str,
    info: IndexInfo | None,
    alias_keys: dict[str, str],
) -> str:
    if info and (info.modrinth_id or info.curseforge_id):
        return stable_key(filename, info)
    return alias_keys.get(filename) or stable_key(filename, info)


def selected_automodpack_modpack(minecraft_dir: Path) -> str | None:
    cfg = minecraft_dir / "automodpack" / "automodpack-client.json"
    if not cfg.is_file():
        return None
    try:
        selected = json.loads(cfg.read_text(encoding="utf-8")).get("selectedModpack", "")
        selected = selected.strip() if isinstance(selected, str) else ""
        return selected or None
    except Exception:
        return None


def discover_automodpack_mods_dirs(minecraft_dir: Path) -> list[Path]:
    modpacks = minecraft_dir / "automodpack" / "modpacks"
    if not modpacks.is_dir():
        return []
    dirs = sorted(
        (entry / "mods" for entry in modpacks.iterdir() if (entry / "mods").is_dir()),
        key=lambda p: str(p),
    )
    selected = selected_automodpack_modpack(minecraft_dir)
    if selected:
        selected_mods = modpacks / selected / "mods"
        if selected_mods.is_dir():
            dirs = [selected_mods] + [d for d in dirs if d != selected_mods]
    return dirs


def jar_modified_ms(path: Path) -> int | None:
    try:
        return int(path.stat().st_mtime * 1000)
    except OSError:
        return None


def select_canonical_mod_jar(
    mods_dir: Path,
    filename: str,
    candidates: list[Path],
) -> Path | None:
    if not candidates:
        return None
    primary = mods_dir / filename
    if primary.is_file():
        return primary
    automodpack = sorted(
        (p for p in candidates if p.is_file() and p != primary),
        key=lambda p: str(p),
    )
    if not automodpack:
        return None
    best = automodpack[0]
    best_mtime = jar_modified_ms(best)
    for path in automodpack[1:]:
        mtime = jar_modified_ms(path)
        replace = False
        if best_mtime is None and mtime is None:
            replace = str(path) < str(best)
        elif best_mtime is None:
            replace = True
        elif mtime is None:
            replace = False
        elif mtime > best_mtime or (mtime == best_mtime and str(path) < str(best)):
            replace = True
        if replace:
            best = path
            best_mtime = mtime
    return best


def scan_local_mods(instance: Path) -> tuple[list[ModEntry], dict[str, str]]:
    instance = instance.resolve()
    mods_dir = instance / "minecraft" / "mods"
    if not mods_dir.is_dir():
        raise SystemExit(f"mods dir not found: {mods_dir}")

    minecraft_dir = mods_dir.parent
    extra_dirs = discover_automodpack_mods_dirs(minecraft_dir)
    tags_path = instance / ".mod-manager" / "mod-tags.json"
    tags = json.loads(tags_path.read_text(encoding="utf-8")) if tags_path.is_file() else {"mods": {}}
    alias_keys = alias_keys_by_filename(tags)
    index = read_index(mods_dir / ".index")

    jars_by_filename: dict[str, list[Path]] = {}
    for directory in [mods_dir, *extra_dirs]:
        if not directory.is_dir():
            continue
        for path in directory.glob("*.jar"):
            if path.name.startswith(".mod-manager-download-"):
                continue
            jars_by_filename.setdefault(path.name, []).append(path)

    mods: list[ModEntry] = []
    for filename in sorted(jars_by_filename):
        candidates = jars_by_filename[filename]
        local_path = select_canonical_mod_jar(mods_dir, filename, candidates)
        if local_path is None:
            continue
        info = index.get(filename)
        key = key_for_file(filename, info, alias_keys)
        tag = tags.get("mods", {}).get(key, {})
        side = resolve_side(tag)
        mods.append(
            ModEntry(
                key=key,
                filename=filename,
                side=side,
                local_path=local_path,
                local_size=local_path.stat().st_size,
            )
        )
    return mods, tags


def mod_applies_to_server_lane(side: str) -> bool:
    return side != "client"


def remote_file_matches(remote: dict[str, int], filename: str, local_size: int) -> bool:
    return remote.get(filename) == local_size


def index_remote_dir(host: str, remote_dir: str) -> dict[str, int]:
    path = remote_dir.replace("\\", "/").replace("'", "''")
    cmd = (
        "powershell -NoProfile -Command "
        f"\"Get-ChildItem -LiteralPath '{path}' -Filter *.jar -File -ErrorAction SilentlyContinue "
        "| ForEach-Object {{ Write-Output ($_.Name + '|' + $_.Length) }}\""
    )
    proc = subprocess.run(
        ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=20", host, cmd],
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or proc.stdout.strip() or "ssh failed")
    files: dict[str, int] = {}
    for line in proc.stdout.splitlines():
        line = line.strip()
        if not line or "|" not in line:
            continue
        name, size_raw = line.rsplit("|", 1)
        try:
            files[name] = int(size_raw.strip())
        except ValueError:
            continue
    return files


@dataclass
class SyncChangeCounts:
    to_update: int
    to_upload: int
    to_delete: int


def classify_sync_changes(pending: list[str], orphans: list[str]) -> tuple[SyncChangeCounts, dict[str, str]]:
    pending = list(pending)
    orphan_action: dict[str, str] = {}
    to_update = 0
    to_delete = 0

    for orphan in orphans:
        key = mod_sync_identity_key(orphan)
        pos = next((i for i, name in enumerate(pending) if mod_sync_identity_key(name) == key), None)
        if pos is not None:
            matched = pending.pop(pos)
            orphan_action[orphan] = f"UPDATE -> {matched}"
            to_update += 1
        else:
            orphan_action[orphan] = "DELETE"
            to_delete += 1

    return SyncChangeCounts(to_update=to_update, to_upload=len(pending), to_delete=to_delete), orphan_action


def main() -> int:
    print(f"Instance: {INSTANCE}")
    print(f"SSH host: {SSH_HOST}")
    print(f"Remote server mods: {REMOTE_SERVER}")
    print()

    mods, _tags = scan_local_mods(INSTANCE)
    server_mods = [m for m in mods if mod_applies_to_server_lane(m.side)]
    allowed = {m.filename for m in server_mods}

    print(f"Local mods total: {len(mods)}")
    print(f"Server lane mods (side != client): {len(server_mods)}")
    print(f"Allowed filenames on server: {len(allowed)}")
    print()

    remote = index_remote_dir(SSH_HOST, REMOTE_SERVER)
    print(f"Remote jars: {len(remote)}")
    print()

    orphans = sorted(name for name in remote if name not in allowed)
    pending = [
        m.filename
        for m in server_mods
        if not remote_file_matches(remote, m.filename, m.local_size)
    ]
    already_synced = len(server_mods) - len(pending)

    counts, orphan_action = classify_sync_changes(pending, orphans)

    print("=== ORPHANS (remote not in allowed) ===")
    for name in orphans:
        size = remote[name]
        action = orphan_action[name]
        identity = mod_sync_identity_key(name)
        print(f"{action:40} {name} ({size} bytes) [key={identity}]")

    print()
    print("=== PENDING UPLOAD/UPDATE (local server lane, name+size mismatch) ===")
    for name in sorted(pending):
        entry = next(m for m in server_mods if m.filename == name)
        remote_size = remote.get(name)
        if remote_size is None:
            print(f"UPLOAD  {name} (local={entry.local_size}, remote=missing)")
        else:
            print(f"UPDATE  {name} (local={entry.local_size}, remote={remote_size})")

    print()
    print("=== UI COUNTS (preview_server_sync_lane server) ===")
    print(f"local:           {len(server_mods)}")
    print(f"remote:          {len(remote)}")
    print(f"already_synced:  {already_synced}")
    print(f"to_upload:       {counts.to_upload}")
    print(f"to_update:       {counts.to_update}")
    print(f"to_delete:       {counts.to_delete}")

    print()
    print("=== DELETE ONLY ===")
    for name in orphans:
        if orphan_action[name] == "DELETE":
            print(f"DELETE  {name} ({remote[name]} bytes)")

    return 0


if __name__ == "__main__":
    sys.exit(main())
