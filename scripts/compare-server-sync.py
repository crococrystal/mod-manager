#!/usr/bin/env python3
import json
import os
import subprocess
import sys
from pathlib import Path

SSH_HOST = os.environ.get("SSH_HOST", "win-test")
INSTANCE = Path(
    os.environ.get(
        "INSTANCE_ROOT",
        "/Users/crococrystal/Minecraft/Crystal Tech",
    )
)
REMOTE_SERVER = "C:/Users/Admin/Desktop/Crystal Tech 1.21.1/mods"
REMOTE_DIST = "C:/Users/Admin/Desktop/Crystal Tech 1.21.1/automodpack/host-modpack/main/mods"
OUT = Path(__file__).resolve().parents[1] / "sync-compare-report.json"


def local_jars(*dirs: Path) -> dict[str, int]:
    out: dict[str, int] = {}
    for d in dirs:
        if not d.is_dir():
            continue
        for p in d.glob("*.jar"):
            if p.name.startswith(".mod-manager-download-"):
                continue
            out[p.name] = p.stat().st_size
    return out


def discover_paths() -> tuple[Path, Path | None]:
    path1 = INSTANCE / "minecraft" / "mods"
    minecraft = INSTANCE / "minecraft"
    path2 = None
    cfg = minecraft / "automodpack" / "automodpack-client.json"
    if cfg.is_file():
        try:
            selected = json.loads(cfg.read_text()).get("selectedModpack", "").strip()
            candidate = minecraft / "automodpack" / "modpacks" / selected / "mods"
            if selected and candidate.is_dir():
                path2 = candidate
        except Exception:
            pass
    if path2 is None:
        modpacks = minecraft / "automodpack" / "modpacks"
        if modpacks.is_dir():
            for d in sorted(modpacks.iterdir()):
                mods = d / "mods"
                if mods.is_dir():
                    path2 = mods
                    break
    return path1, path2


def remote_jars(remote_dir: str) -> dict[str, int]:
    ps = remote_dir.replace("/", "\\")
    cmd = (
        "powershell -NoProfile -Command "
        f"\"Get-ChildItem -LiteralPath '{ps}' -Filter *.jar -File -ErrorAction SilentlyContinue "
        "| ForEach-Object {{ Write-Output ($_.Name + '|' + $_.Length) }}\""
    )
    proc = subprocess.run(
        ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=20", SSH_HOST, cmd],
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or proc.stdout.strip() or "ssh failed")
    out: dict[str, int] = {}
    for line in proc.stdout.splitlines():
        line = line.strip()
        if not line or "|" not in line:
            continue
        name, size = line.rsplit("|", 1)
        try:
            out[name] = int(size)
        except ValueError:
            pass
    return out


def fmt_size(n: int) -> str:
    if n < 1024:
        return f"{n} B"
    kb = n / 1024
    if kb < 1024:
        return f"{kb:.1f} KB"
    mb = kb / 1024
    return f"{mb:.2f} MB"


def compare(label: str, remote_dir: str, p1: Path, p2: Path | None) -> dict:
    m1 = local_jars(p1)
    m2 = local_jars(p2) if p2 else {}
    union = {**m2, **m1}
    remote = remote_jars(remote_dir)
    upload = sorted(set(union) - set(remote))
    delete = sorted(set(remote) - set(union))
    mismatch = sorted(n for n in set(union) & set(remote) if union[n] != remote[n])
    synced = sorted(n for n in set(union) & set(remote) if union[n] == remote[n])
    return {
        "lane": label,
        "remote_dir": remote_dir,
        "path1": str(p1),
        "path2": str(p2) if p2 else None,
        "counts": {
            "local": len(union),
            "remote": len(remote),
            "synced": len(synced),
            "upload": len(upload),
            "delete": len(delete),
            "mismatch": len(mismatch),
        },
        "upload": [{"name": n, "size": union[n], "size_human": fmt_size(union[n])} for n in upload],
        "delete": [{"name": n, "size": remote[n], "size_human": fmt_size(remote[n])} for n in delete],
        "mismatch": [
            {
                "name": n,
                "local_size": union[n],
                "remote_size": remote[n],
                "local_human": fmt_size(union[n]),
                "remote_human": fmt_size(remote[n]),
            }
            for n in mismatch
        ],
    }


def main() -> int:
    p1, p2 = discover_paths()
    report = {
        "instance": str(INSTANCE),
        "ssh_host": SSH_HOST,
        "lanes": [
            compare("distribution", REMOTE_DIST, p1, p2),
            compare("server", REMOTE_SERVER, p1, p2),
        ],
    }
    OUT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(str(OUT))
    for lane in report["lanes"]:
        print(f"\n=== {lane['lane'].upper()}: ONLY ON REMOTE (нет локально, сотрутся) ===")
        for item in lane["delete"]:
            print(item["name"])
        print(f"count: {len(lane['delete'])}")
        print(f"\n=== {lane['lane'].upper()}: ONLY ON LOCAL (нет на сервере, зальются) ===")
        for name in lane["upload"][:30]:
            print(name["name"])
        if len(lane["upload"]) > 30:
            print(f"... ещё {len(lane['upload']) - 30}")
        print(f"count: {len(lane['upload'])}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
