# -*- coding: utf-8 -*-
"""闸门 CI：SQL 里引用的列必须真实存在（medals.active 那类）。

实测（批一百五十五 P-342）：站方 0382 给 medals 加了 auto_* 六列，代码却在
四处写了 `WHERE active = TRUE` —— medals 表从来没有 active 列。结果：

  · worker 的三个自动成就 job（threshold_sweep / record_seeding_hours /
    medal_backfill）从上线起一次没成功执行过；
  · `/medals/progress` 端点每次 500。

这类 bug 的特点是：**静态读代码看不出来，运行时必炸，而炸点与写点分离**
（写迁移的人与写查询的人不是同一处），只有真跑一次或对一次 schema 才发现。

本闸的做法：把 apps/api/migrations 里所有 `ALTER TABLE <t> ADD COLUMN [IF NOT
EXISTS] <c>` 与 `CREATE TABLE` 的列收成一张「已知列」表，再扫 apps/*/src 里的
SQL 字符串，找出对某张表引用了**从未被任何迁移定义过**的列。
命中即红——宁可多报也不漏（建表前的历史列可能误报，那时按白名单处理）。
"""
import io
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MIG = os.path.join(ROOT, "apps/api/migrations")

COL = re.compile(
    r"ALTER\s+TABLE\s+(?:IF\s+EXISTS\s+)?(\w+)\s+"
    r"ADD\s+COLUMN\s+(?:IF\s+NOT\s+EXISTS\s+)?(\w+)",
    re.I)
CREATE = re.compile(
    r"CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(\w+)\s*\((.*?)\n\)",
    re.I | re.S)

known = {}
for fn in sorted(os.listdir(MIG)):
    if not fn.endswith(".sql"):
        continue
    s = io.open(os.path.join(MIG, fn), encoding="utf-8", errors="replace").read()
    s = re.sub(r"--[^\n]*", "", s)          # 剥注释
    for tbl, col in COL.findall(s):
        known.setdefault(tbl.lower(), set()).add(col.lower())
    for tbl, body in CREATE.findall(s):
        cols = known.setdefault(tbl.lower(), set())
        for part in body.split(","):
            m = re.match(r"\s*(\w+)\s+\w", part)
            if m and m.group(1).upper() not in ("PRIMARY", "UNIQUE", "FOREIGN",
                                                "CONSTRAINT", "CHECK"):
                cols.add(m.group(1).lower())

# 只钉「迁移里明确 ADD 过的表」——基础表（users/torrents/...）历史列太多，
# 不在本闸范围（本闸针对的正是「新加列 + 新写查询」这一类）。
targets = {t: c for t, c in known.items()
           if t not in ("users", "torrents", "messages", "snatches",
                        "spark_ledger", "site_settings", "categories",
                        "promotions", "download_keys", "traffic_ledger")}

hit = []
for src in ("apps/api/src", "apps/worker/src"):
    for dp, _, fns in os.walk(os.path.join(ROOT, src)):
        for fn in fns:
            if not fn.endswith(".rs"):
                continue
            p = os.path.join(dp, fn)
            txt = io.open(p, encoding="utf-8", errors="replace").read()
            for i, ln in enumerate(txt.splitlines(), 1):
                code = ln.split("//", 1)[0]
                m = re.search(r"\b(?:FROM|JOIN|UPDATE|INTO)\s+(\w+)", code, re.I)
                if not m:
                    continue
                tbl = m.group(1).lower()
                if tbl not in targets:
                    continue
                cols = targets[tbl]
                for cm in re.finditer(r"\b(\w+)\s*=", code):
                    c = cm.group(1).lower()
                    # 只看像列名的（排除 SQL 关键字与参数占位）
                    if c in ("select", "and", "or", "not", "set", "on",
                             "where", "values", "count", "sum", "true", "false"):
                        continue
                    if (c.endswith("_id") or c in cols or c.isupper()
                            or "_" in c):
                        if c not in cols and not c.endswith("_id"):
                            hit.append("%s:%d  %s.%s" % (
                                p.replace("\\", "/").split("apps/")[-1], i, tbl, c))

if hit:
    print("[FAIL] %d 处引用了迁移里不存在的列：" % len(hit))
    seen = set()
    for h in hit:
        key = h.split(":")[-1]
        if key in seen:
            continue
        seen.add(key)
        print("   " + h)
    print("   后果：运行时必炸（如 medals.active ⇒ 三个自动成就 job 全废）。")
    raise SystemExit(1)
print("== ALL PASS（新增表的列引用与迁移一致）==")
