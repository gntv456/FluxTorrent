"""一次性审计：逐条比对 _sqlx_migrations 存档 checksum 与工作区迁移文件的 LF/CRLF 口径。
sqlx (0.8) 存 sha384；判定每条是 LF、CRLF 还是内容漂移。只读，不写库。"""
import hashlib
import os
import subprocess

rows = subprocess.run(
    ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d", "fluxtorrent", "-t", "-A", "-c",
     "SELECT version, encode(checksum,'hex') FROM _sqlx_migrations ORDER BY version"],
    capture_output=True, text=True).stdout.strip().splitlines()
db = {}
for r in rows:
    if "|" in r:
        v, c = r.split("|")
        db[int(v)] = c

bad = []
for name in sorted(os.listdir("apps/api/migrations")):
    if not name.endswith(".sql"):
        continue
    ver = int(name[:4])
    data = open(os.path.join("apps/api/migrations", name), "rb").read()
    lf = hashlib.sha384(data).hexdigest()
    crlf = hashlib.sha384(data.replace(b"\n", b"\r\n")).hexdigest()
    stored = db.pop(ver, None)
    if stored is None:
        bad.append((ver, "APPLIED-IN-DB-BUT-FILE-MISSING", name))
    elif stored == lf:
        pass
    elif stored == crlf:
        bad.append((ver, "DB=CRLF", name))
    else:
        bad.append((ver, "CONTENT-MISMATCH", name))
for ver in sorted(db):
    bad.append((ver, "FILE-MISSING-IN-WORKTREE", f"(db only, {db[ver][:12]}…)"))

print("total applied in db:", len(rows))
if bad:
    print("problem rows:")
    for b in bad:
        print("  ", b)
else:
    print("all applied migrations match working-tree LF checksums")
