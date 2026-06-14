#!/usr/bin/env python3
"""Local + SSH: show exactly what server preview will delete."""
import json, os, re, subprocess, sys
from pathlib import Path

INSTANCE = Path(os.environ.get("INSTANCE_ROOT", "/Users/crococrystal/Minecraft/Crystal Tech"))
REMOTE = "C:/Users/Admin/Desktop/Crystal Tech 1.21.1/mods"
SSH = os.environ.get("SSH_HOST", "win-test")

# --- mod_names (minimal) ---
def strip_filename_decorations(value):
    value = value.strip()
    while value and (value[0].isspace() or value[0] in "-_+()[]"):
        value = value[1:]
    return value.strip()

def is_ver(seg):
    s = seg.lower()
    if s in {"neoforge","forge","fabric","quilt","client","server","universal","both"}: return True
    if s.startswith("mc") and len(s)>2 and all(c.isdigit() or c=='.' for c in s[2:]): return True
    if s.startswith("v") and len(s)>1 and s[1].isdigit(): return True
    if any(c.isdigit() for c in s) and ('.' in s or '+' in s) and len(s) <= 16: return True
    return False

def identity_key(filename):
    stem = filename[:-4] if filename.endswith(".jar") else filename
    clean = strip_filename_decorations(stem)
    parts = [p for p in re.split(r"[-_]", clean) if p]
    while len(parts) > 1 and is_ver(parts[-1]):
        parts.pop()
    base = parts[0] if len(parts)==1 else "-".join(parts) if parts else clean
    return re.sub(r"[^a-z0-9]", "", base.lower())

def normalize_side(s):
    s = (s or "").strip().lower()
    return s if s in ("client","server") else "universal"

def resolve_side(tag):
    if tag.get("labelOverrides",{}).get("sideMode","").strip() == "manual":
        return normalize_side(tag.get("side","universal"))
    pl = tag.get("providerLabels",{})
    if not pl.get("fetchedAt","").strip():
        return "universal"
    m = {
        ("required","unsupported"): "client",
        ("unsupported","required"): "server",
    }
    c, s = pl.get("clientSide",""), pl.get("serverSide","")
    return normalize_side(m.get((c,s), "universal"))

def local_jars():
    dirs = [INSTANCE / "minecraft" / "mods"]
    mp = INSTANCE / "minecraft" / "automodpack" / "modpacks"
    cfg = INSTANCE / "minecraft" / "automodpack" / "automodpack-client.json"
    if cfg.is_file():
        sel = json.loads(cfg.read_text()).get("selectedModpack","").strip()
        if sel and (mp/sel/"mods").is_dir():
            dirs.append(mp/sel/"mods")
    elif mp.is_dir():
        for d in sorted(mp.iterdir()):
            if (d/"mods").is_dir():
                dirs.append(d/"mods")
                break
    out = {}
    for d in dirs:
        for p in d.glob("*.jar"):
            if p.name.startswith(".mod-manager-download-"):
                continue
            out[p.name] = p.stat().st_size
    return out

def remote_jars():
    ps = REMOTE.replace("/", "\\")
    cmd = (
        f'powershell -NoProfile -Command '
        f'"Get-ChildItem -LiteralPath \'{ps}\' -Filter *.jar -File -ErrorAction SilentlyContinue '
        f'| ForEach-Object {{ Write-Output ($_.Name + \'|\' + $_.Length) }}"'
    )
    r = subprocess.run(["ssh","-o","BatchMode=yes",SSH,cmd], capture_output=True, text=True, timeout=60)
    if r.returncode != 0:
        print("SSH ERROR:", r.stderr or r.stdout, file=sys.stderr)
        sys.exit(1)
    out = {}
    for line in r.stdout.splitlines():
        line = line.strip()
        if "|" not in line: continue
        n, sz = line.rsplit("|", 1)
        try: out[n] = int(sz)
        except: pass
    return out

def main():
    tags = json.loads((INSTANCE/".mod-manager/mod-tags.json").read_text())
    alias = {}
    for key, tag in tags.get("mods",{}).items():
        for a in tag.get("aliases",[]):
            if a.strip() and a not in alias:
                alias[a.strip()] = (key, tag)

    local = local_jars()
    side_by_name = {}
    for name in local:
        key, tag = alias.get(name, (f"manual:{name}", {}))
        if name not in alias:
            tag = tags.get("mods",{}).get(key, {})
        side_by_name[name] = resolve_side(tag)

    allowed = {n for n,s in side_by_name.items() if s != "client"}
    client = sorted(n for n,s in side_by_name.items() if s == "client")

    remote = remote_jars()
    orphans = sorted(n for n in remote if n not in allowed)
    pending = sorted(n for n in allowed if n not in remote or remote[n] != local[n])

    pending_copy = list(pending)
    pure_delete = []
    updates = []
    for o in orphans:
        k = identity_key(o)
        pos = next((i for i,p in enumerate(pending_copy) if identity_key(p)==k), None)
        if pos is not None:
            updates.append((o, pending_copy.pop(pos)))
        else:
            pure_delete.append(o)

    synced = len(allowed) - len(pending)
    print(f"Локально всего: {len(local)}")
    print(f"Server lane (не client): {len(allowed)}")
    print(f"Client-моды локально ({len(client)}):")
    for n in client: print(f"  {n}")
    print(f"На сервере jar: {len(remote)}")
    print(f"Совпадают (preview already_synced ~): {len(allowed)-len(pending)}")
    print(f"Будет обновлено: {len(updates)}")
    print(f"Будет отправлено: {len(pending_copy)}")
    print(f"Будет удалено (чистое): {len(pure_delete)}")
    print()
    if updates:
        print("=== ОБНОВЛЕНИЯ (старое на сервере → новое локально) ===")
        for old, new in updates:
            print(f"  {old}")
            print(f"    → {new}")
    print()
    print("=== УДАЛИТСЯ (нет локальной замены) ===")
    if pure_delete:
        for n in pure_delete:
            print(f"  {n}")
    else:
        print("  (нет)")
    print()
    print("=== Только на сервере по ИМЕНИ (все лишние jar) ===")
    only_remote = sorted(set(remote) - set(local))
    for n in only_remote:
        mark = " ← УДАЛИТСЯ" if n in pure_delete else (" ← старая версия" if any(o==n for o,_ in updates) else "")
        print(f"  {n}{mark}")

if __name__ == "__main__":
    main()
