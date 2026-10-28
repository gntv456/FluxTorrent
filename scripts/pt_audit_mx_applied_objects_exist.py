#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""MX：`_sqlx_migrations` 说「应用过」⇒ 它声明的对象必须真在库里（P-461 常驻）。

为什么这条必须有（2026-10-28 现网取证）：0424/0425 两行台账 success=t、
checksum 与磁盘文件逐字节一致，可它们声明的三张表、一个函数、六把设定键
**一个都不在**。现网症状是硬的：`GET /contracts/tiers`、`/contracts/mine`
各回 500(code=1000)；`job_status.dormant_mark.last_ok=f`；热搜词读写两头
分别被 `let _ =` 与 `unwrap_or_default()` 吞成「静默缺席」。
而本仓原有的 LV 闸门只盯 `site_settings` 的键，所以「表没建出来」这一型
从它眼皮底下整条过去了——**尺子的覆盖面就是洞的形状**。

判据不认「谁写的行」：同一把 record_migration.py 登记的 0423，它的
`promotions.announced_at` 确实在库里（先执行后登记，用法正确）。所以只认
一个事实：声明过 ⇒ 必须在；后续迁移又 DROP/DELETE 掉的从宽扣除。

台账 LEDGER = 「当前已知、等待补件上线」的缺失。多一条=新分叉，
少一条=补件已生效、必须同步删号。0427 部署后这里应当清空。
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402
from record_migration import SAFE, declared, probe  # noqa: E402

MIG = "apps/api/migrations"
PROBE = ("SELECT x.lbl FROM (SELECT k || '|' || n AS lbl, CASE"
         " WHEN k IN ('rel','tbl') THEN to_regclass(n) IS NOT NULL"
         " WHEN k = 'func' THEN EXISTS (SELECT 1 FROM pg_proc"
         "                                WHERE proname = n)"
         " WHEN k = 'typ' THEN EXISTS (SELECT 1 FROM pg_type"
         "                              WHERE typname = n)"
         " WHEN k = 'key' THEN EXISTS (SELECT 1 FROM site_settings"
         "                                WHERE name = n)"
         " WHEN k = 'col' THEN EXISTS (SELECT 1"
         "       FROM information_schema.columns"
         "       WHERE lower(table_name) = lower(split_part(n, '.', 1))"
         "         AND lower(column_name) = lower(split_part(n, '.', 2)))"
         " ELSE FALSE END AS present FROM (VALUES\n%s\n) v(k, n)"
         ") x WHERE NOT x.present")

# 2026-10-28 13:00+08：站长在同一棵树上接受 0425 的修法（索引谓词去 now()）并把
# 0427 执行后登记（record_migration.py，checksum 按 blob 字节，audit 实测 420/423
# 对得上、0 问题）。现网实测 tiers=5 / keys=6 / 乘数 4:2.5:1.5:1 ⇒ 本条台账按棘轮
# 清空：再出现任何「登记说跑过、对象却不在」都是新分叉。
LEDGER = {}


def read(v):
    for f in sorted(os.listdir(MIG)):
        if f.endswith(".sql") and int(f[:4]) == v:
            return open(os.path.join(MIG, f), encoding="utf-8",
                        errors="replace").read()
    return ""


def dropped_in(text):
    """后续迁移删掉的东西：关系/类型名、列名、设定键，三类分开收。"""
    rels = {m.group(1).split(".")[-1].lower() for m in re.finditer(
        r"DROP\s+(?:TABLE|SEQUENCE|VIEW|INDEX|TYPE|FUNCTION)\s+"
        r"(?:IF\s+EXISTS\s+)?([A-Za-z_][A-Za-z0-9_\.]*)", text, re.I)}
    cols = {m.group(1).lower() for m in re.finditer(
        r"DROP\s+COLUMN\s+(?:IF\s+EXISTS\s+)?([A-Za-z_][A-Za-z0-9_]*)",
        text, re.I)}
    # 停读删键的写法是 `DELETE FROM site_settings WHERE name LIKE
    # 'games_scratch_%_pct'`（0249 实测）：只按字面名豁免会把这批合法退役的
    # 键读成「对象缺失」，所以 LIKE 形态要翻成正则一起判。
    pats = []
    for m in re.finditer(r"DELETE\s+FROM\s+(?:site_settings|settings_meta)"
                         r"[^;]*;", text, re.I | re.S):
        for lit in re.findall(r"'([A-Za-z0-9_%\.]+)'", m.group(0)):
            pats.append("^" + "".join(".*" if c == "%" else
                                      ("." if c == "_" else re.escape(c))
                                      for c in lit) + "$")
    return rels, cols, set(pats)


inst = {}
for ln in psql_stdin("SELECT version, success FROM _sqlx_migrations"
                     ).splitlines():
    if "|" in ln:
        v, s = ln.split("|")
        inst[int(v)] = s.strip() == "t"
versions = {int(f[:4]) for f in os.listdir(MIG) if f.endswith(".sql")}
checked = sorted(v for v in inst if inst[v] and v in versions)

per_ver = {}
for v in checked:
    per_ver[v] = [(k, n) for k, n in declared(read(v))
                  if SAFE.match(n.split(".")[0])]

texts = {v: read(v) for v in checked}
wanted = {}  # "kind|name" -> 声明它的最小版本
for v in checked:
    rels, cols, keys = set(), set(), set()
    for w, t in texts.items():
        if w > v:
            r, c, k = dropped_in(t)
            rels |= r
            cols |= c
            keys |= k
    for kind, name in per_ver[v]:
        low = name.lower()
        if kind in ("rel", "tbl", "func", "typ") and low in rels:
            continue
        if kind == "key" and any(re.match(p, low, re.I) for p in keys):
            continue
        if kind == "col" and low.split(".")[1] in cols:
            continue
        lbl = "%s|%s" % (kind, name)
        wanted.setdefault(lbl, v)

rows = ["('%s','%s')" % tuple(lbl.split("|", 1)) for lbl in sorted(wanted)]
seen_missing = sorted({ln.strip() for ln in
                       psql_stdin(PROBE % ",\n".join(rows)).splitlines()
                       if ln.strip()})

ghost = declared("CREATE TABLE IF NOT EXISTS mx_probe_ghost (\n"
                 "  id BIGINT PRIMARY KEY);\n"
                 "CREATE OR REPLACE FUNCTION mx_probe_ghost_fn(a int)\n"
                 "  RETURNS int LANGUAGE sql AS $$ SELECT 1 $$;\n"
                 "ALTER TABLE mx_probe_ghost ADD COLUMN mx_probe_col int;\n"
                 "INSERT INTO site_settings (name, value) "
                 "VALUES ('mx_probe_key', '1')\n"
                 "ON CONFLICT (name) DO NOTHING;\n"
                 "CREATE TYPE mx_probe_enum AS ENUM ('a');\n")
ok("MX1 正控：解析器要能吃下五种形状（表/函数/列/键/枚举）——少一种就是瞎",
   {("rel", "mx_probe_ghost"), ("func", "mx_probe_ghost_fn"),
    ("col", "mx_probe_ghost.mx_probe_col"), ("key", "mx_probe_key"),
    ("typ", "mx_probe_enum")} <= set(ghost), "解析=%s" % sorted(ghost))
got = psql_stdin(PROBE % ("('rel','mx_probe_ghost'),"
                          "('func','mx_probe_ghost_fn'),"
                          "('col','mx_probe_ghost.mx_probe_col'),"
                          "('key','mx_probe_key'),"
                          "('typ','mx_probe_enum')")).splitlines()
ok("MX1b 正控：同一支探针 SQL 必须把这 5 个假对象全判成缺失",
   len([g for g in got if g.strip()]) == 5, "实回=%s" % got)

for lbl in seen_missing:
    print("   [缺 %04d] %s" % (wanted[lbl], lbl))
actual = {(wanted[lbl], lbl) for lbl in seen_missing}
ok("MX2 「登记说跑过、对象却不在」= 台账 %d 条（补件上线后须清空）"
   % len(LEDGER), actual == set(LEDGER),
   "实测 %d 条=%s" % (len(actual), sorted(actual)))
for (v, lbl), why in sorted(LEDGER.items()):
    print("   [台账 %04d] %-42s %s" % (v, lbl, why))

ok("MX3 反空转：受检 applied 版本 > 100 且可比对象 > 400"
   "（解析坏会把集合读成空集而假绿）",
   len(checked) > 100 and len(wanted) > 400,
   "版本=%d 对象=%d" % (len(checked), len(wanted)))

n24 = {n for _, n in declared(read(424))}
n25 = {n for _, n in declared(read(425))}
n27 = {n for _, n in declared(read(427))}
ok("MX4 补件覆盖：0427 声明须含 0424 与 0425 的全部对象（补一半比不补更坏）",
   bool(n24 or n25) and not ((n24 | n25) - n27),
   "缺=%s 0427=%d 项" % (sorted((n24 | n25) - n27), len(n27)))

miss_ghost = probe([("rel", "mx_probe_ghost"),
                     ("func", "mx_probe_ghost_fn")])
miss_real = probe([("rel", "seed_contracts"),
                   ("func", "seeding_publisher_mult")])
ok("MX5 工具门：同一支探针必须对合成 ghost 判缺、对已建对象判在"
   "（既不是恒绿也不是恒红；登记工具用的就是它）",
   set(miss_ghost) == {"rel|mx_probe_ghost", "func|mx_probe_ghost_fn"}
   and miss_real == [], "ghost=%s real=%s" % (miss_ghost, miss_real))

summary()
