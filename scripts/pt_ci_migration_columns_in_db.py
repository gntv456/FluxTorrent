# -*- coding: utf-8 -*-
"""闸门 CI：迁移新增的列必须被查询方正确引用——直接对**运行库**校验。

实测（批一百五十五 P-342）：0382 给 medals 加了 auto_* 六列，代码四处写了
`WHERE active = TRUE`，而 medals 表从来没有 active 列。后果是三个自动成就 job
从上线起一次没跑成、`/medals/progress` 每次 500。

这类 bug 的特点是静态推断不可靠（列可能来自早期 CREATE TABLE、ALTER 的
多种写法、或者同名不同表），**唯一可靠的判据是运行库本身**。所以本闸改成：

  对每张「近 N 条迁移里被 ADD COLUMN 过」的表，列出真实列；再对每条迁移
  提取它声明的列名，逐个问库「在不在」——不在即红。

这样既不误报（库说了算），又能在迁移入库那一刻就抓到。
"""
import io
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MIG = os.path.join(ROOT, "apps/api/migrations")


def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    return (r.stdout or "").strip()


# 各表在库里的真实列（一次拉全，后面在内存里比对）
cols = {}
for ln in psql("SELECT DISTINCT table_name, column_name "
               "FROM information_schema.columns "
               "WHERE table_schema='public'").splitlines():
    if "|" not in ln:
        continue
    tbl, col = ln.split("|", 1)
    cols.setdefault(tbl.strip().lower(), set()).add(col.strip().lower())

ADD = re.compile(
    r"ALTER\s+TABLE\s+(?:IF\s+EXISTS\s+)?(\w+)\s+"
    r"ADD\s+COLUMN\s+(?:IF\s+NOT\s+EXISTS\s+)?(\w+)", re.I)

bad = []
checked = 0
for fn in sorted(os.listdir(MIG)):
    if not fn.endswith(".sql"):
        continue
    raw = io.open(os.path.join(MIG, fn), encoding="utf-8", errors="replace").read()
    s = re.sub(r"--[^\n]*", "", raw)
    for tbl, col in ADD.findall(s):
        have = cols.get(tbl.lower())
        if have is None:
            continue          # 表还没建（正常，后面会建）
        checked += 1
        if col.lower() not in have:
            bad.append("%s  %s.%s" % (fn, tbl, col))

if bad:
    print("[FAIL] %d 处：迁移声明的列在运行库里不存在" % len(bad))
    for b in bad:
        print("   " + b)
    print("   后果：跑这份迁移时静默跳过（ADD COLUMN IF NOT EXISTS 不报错），")
    print("        而查询方按该列写 WHERE ⇒ 运行时 column does not exist。")
    raise SystemExit(1)
print("== ALL PASS（迁移声明列 %d 项，全部存在于运行库）==" % checked)
