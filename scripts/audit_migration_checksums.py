"""一次性审计：逐条比对 _sqlx_migrations 存档 checksum 与工作区迁移文件的 LF/CRLF 口径。
sqlx (0.8) 存 sha384；判定每条是 LF、CRLF 还是内容漂移。只读，不写库。

两类结果分开看：
  问题（退出码 1）＝ 内容漂移 / 库里以 CRLF 记账 / 库里有记账但工作树已无该文件
  信息（不算失败）＝ 工作树有文件但库里未记账 = 待应用的新迁移，属正常状态

本次修正的两处自身缺陷：① SQL 拼接漏空格（"ORDER BY" + "version" → ORDER BYversion），
psql 报错被静默吞掉，rows 为空 → 每条迁移都被误判成「APPLIED-IN-DB-BUT-FILE-MISSING」；
② 那两个分支标签写反了（文件在库里无 = 待应用；库里有记账文件没了 才是缺文件）。
"""
import hashlib
import os
import subprocess
import sys

res = subprocess.run(
    ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-d",
        "fluxtorrent", "-t", "-A", "-c",
     "SELECT version, encode(checksum,'hex') FROM _sqlx_migrations "
     "ORDER BY version"],
    capture_output=True, text=True, encoding="utf-8", errors="replace")
if res.returncode:
    raise SystemExit("读取 _sqlx_migrations 失败：" + res.stderr.strip()[:200])
rows = res.stdout.strip().splitlines()
db = {}
for r in rows:
    if "|" in r:
        v, c = r.split("|")
        db[int(v)] = c

bad, pending = [], []
for name in sorted(os.listdir("apps/api/migrations")):
    if not name.endswith(".sql"):
        continue
    ver = int(name[:4])
    data = open(os.path.join("apps/api/migrations", name), "rb").read()
    lf = hashlib.sha384(data).hexdigest()
    crlf = hashlib.sha384(data.replace(b"\n", b"\r\n")).hexdigest()
    stored = db.pop(ver, None)
    if stored is None:
        pending.append((ver, name))
    elif stored == lf:
        pass
    elif stored == crlf:
        bad.append((ver, "DB=CRLF", name))
    else:
        bad.append((ver, "CONTENT-MISMATCH", name))
for ver in sorted(db):
    bad.append((ver, "APPLIED-IN-DB-BUT-FILE-MISSING",
               f"(db only, {db[ver][:12]}…)"))

print("库里已应用: %d 条 / 工作树待应用: %d 条" % (len(rows), len(pending)))
if pending:
    print("  待应用: " + ", ".join(str(v) for v, _ in pending[-8:]))
if bad:
    print("problem rows:")
    for b in bad:
        print("  ", b)
    sys.exit(1)
print("all applied migrations match working-tree LF checksums")
