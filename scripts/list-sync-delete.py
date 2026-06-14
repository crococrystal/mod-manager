#!/usr/bin/env python3
import json
import re
import subprocess
import sys
from pathlib import Path

SSH = "win-test"
INSTANCE = Path(
    "/Users/crococrystal/Library/Application Support/PrismLauncher/instances/Crystal Tech 1.21.1"
)
REMOTE_SERVER = "C:/Users/Admin/Desktop/Crystal Tech 1.21.1/mods"
REMOTE_DIST = "C:/Users/Admin/Desktop/Crystal Tech 1.21.1/automodpack/host-modpack/main/mods"


def remote_jars(remote_dir: str) -> dict[str, int]:
    ps = remote_dir.replace("/", "\\")
    cmd = (
        "powershell -NoProfile -Command "
        f"\"Get-ChildItem -LiteralPath '{ps}' -Filter *.jar -File -ErrorAction SilentlyContinue "
        "| ForEach-Object {{ $_.Name + '|' + $_.Length }}\""
    )
    proc = subprocess.run(
        ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=20", SSH, cmd],
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or proc.stdout.strip() or "ssh failed")
    out: dict[str, int] = {}
    for line in proc.stdout.splitlines():
        line = line.strip()
        if "|" not in line:
            continue
        name, size = line.rsplit("|", 1)
        try:
            out[name] = int(size)
        except ValueError:
            pass
    return out


def local_union() -> dict[str, int]:
    p1 = INSTANCE / "minecraft/mods"
    cfg = json.loads((INSTANCE / "minecraft/automodpack/automodpack-client.json").read_text())
    sel = cfg.get("selectedModpack", "").strip()
    p2 = INSTANCE / "minecraft/automodpack/modpacks" / sel / "mods"
    out: dict[str, int] = {}
    for d in (p2, p1):
        if not d.is_dir():
            continue
        for p in d.glob("*.jar"):
            if p.name.startswith(".mod-manager-download-"):
                continue
            out[p.name] = p.stat().st_size
    return out


def alias_side_map() -> dict[str, str]:
    path = INSTANCE / ".mod-manager/mod-tags.json"
    if not path.is_file():
        return {}
    tags = json.loads(path.read_text())
    out: dict[str, str] = {}
    for tag in tags.get("mods", {}).values():
        side = tag.get("side", "universal")
        for alias in tag.get("aliases", []):
            out[alias] = side
    return out


def identity_key(filename: str) -> str:
    stem = filename[:-4] if filename.endswith(".jar") else filename
    parts = re.split(r"[-_]", stem)
    loaders = {
        "neoforge",
        "forge",
        "fabric",
        "quilt",
        "client",
        "server",
        "universal",
        "both",
    }

    def is_ver(seg: str) -> bool:
        s = seg.lower()
        if s in loaders or s.startswith("mc"):
            return True
        if re.match(r"^v?\d", s):
            return True
        if any(c.isdigit() for c in s) and ("." in s or "+" in s):
            return True
        return False

    while parts and is_ver(parts[-1]):
        parts.pop()
    return re.sub(r"[^a-z0-9]", "", "".join(parts).lower())


def classify(pending: list[str], orphans: list[str]) -> dict:
    pending = list(pending)
    updates = []
    delete = []
    for orphan in orphans:
        key = identity_key(orphan)
        pos = next((i for i, p in enumerate(pending) if identity_key(p) == key), None)
        if pos is not None:
            updates.append({"old": orphan, "new": pending.pop(pos)})
        else:
            delete.append(orphan)
    return {"update": updates, "upload": pending, "delete": delete}


def lane(label: str, remote_dir: str, allowed: dict[str, int]) -> dict:
    remote = remote_jars(remote_dir)
    allowed_names = set(allowed)
    orphans = sorted(set(remote) - allowed_names)
    pending = [
        name
        for name in sorted(allowed)
        if name not in remote or remote[name] != allowed[name]
    ]
    synced = len(allowed_names & set(remote)) - len(
        [n for n in allowed_names & set(remote) if remote[n] != allowed[n]]
    )
    changes = classify(pending, orphans)
    return {
        "lane": label,
        "remote_dir": remote_dir,
        "counts": {
            "local": len(allowed),
            "remote": len(remote),
            "synced": synced,
            "update": len(changes["update"]),
            "upload": len(changes["upload"]),
            "delete": len(changes["delete"]),
        },
        **changes,
        "delete_sizes": {n: remote[n] for n in changes["delete"]},
    }


def main() -> int:
    union = local_union()
    sides = alias_side_map()
    server_allowed = {
        n: s for n, s in union.items() if sides.get(n, "universal") != "client"
    }
    report = {
        "instance": str(INSTANCE),
        "lanes": [
            lane("distribution", REMOTE_DIST, union),
            lane("server", REMOTE_SERVER, server_allowed),
        ],
    }
    for lane_data in report["lanes"]:
        print(
            f"{lane_data['lane']}: delete={lane_data['counts']['delete']} "
            f"update={lane_data['counts']['update']} upload={lane_data['counts']['upload']}"
        )
        for name in lane_data["delete"]:
            print(f"  DELETE {name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
