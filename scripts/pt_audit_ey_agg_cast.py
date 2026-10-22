# -*- coding: utf-8 -*-
"""闸门 EY：解码侧聚合的类型口径（P-535 面板池子 / P-536 流量审计）。

为什么钉这个家族：PG 里 SUM(bigint 列) 的结果是 **numeric**，而 sqlx 的
i64 只认 int2/int4/int8 —— 于是不带 ::bigint 的聚合读数必定解码失败。失败
之后有两种死法，都见过：unwrap_or(0) 把它演成「池子是空的」（面板 200/code=0，
读数撒谎），`.await?` 把它演成整个作业报错（cheat_audit 只在真的抓到刷流量
者那天才炸 —— 最需要告警落库的那一次）。

  EY1 前提：用 pg_typeof 钉住「整型宽度决定 SUM 的结果类型」，让 cast 不被
      后人当多余东西删掉。
  EY2 活体：/admin/economy-dashboard 的 pool_balance 必须等于真库
      SUM(amount)::bigint；前置要求真库读数 > 0（空集合会让「等于 0」假绿）。
  EY3 活体：阀门键被后台清成空串时面板必须回 0 而不是 500（NULLIF 那一支），
      跑完原值写回并回读。
  EY4 点名：dashboard / worker audit 两处必须带 cast；站方在途的
      forum_reward.granted_by_idem 只钉桩记现网读数，不判红。
  EY5 棘轮：全仓「解码侧聚合缺 cast」的命中数不得多于基线 6 处（逐条按 PG
      类型定性过：integer 列的 SUM 本就是 bigint、INSERT/UPDATE 侧不解码）。

跑法：
    PYTHONIOENCODING=utf-8 python scripts/pt_audit_ey_agg_cast.py
"""
import io
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import (  # noqa: E402
    BASE, call, login, ok, psql_stdin, summary)

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def q(sql):
    return psql_stdin(sql.rstrip(";") + ";").strip()


def src(rel):
    p = os.path.join(REPO, *rel.split("/"))
    return io.open(p, encoding="utf-8", errors="replace").read().replace(
        "\r\n", "\n")


def no_comments(text):
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return re.sub(r"//[^\n]*", "", text)


print("== EY 解码侧聚合的类型口径 ==")
print("   基线 %s" % BASE)

# ---------- EY1 前提：类型事实 ----------
ty = {}
for label, sql in (
    ("sum_bigint", "SELECT pg_typeof(sum(amount)) FROM spark_ledger"),
    ("sum_int", "SELECT pg_typeof(sum(seeders)) FROM torrents"),
    ("avg_int", "SELECT pg_typeof(avg(seeded_seconds)) FROM snatches"),
):
    ty[label] = q(sql)
ok("EY1 前提钉死：SUM(bigint)=numeric、SUM(int4)=bigint、AVG=numeric（"
   "numeric 解不进 i64 ⇒ cast 不是装饰）",
   ty["sum_bigint"] == "numeric" and ty["sum_int"] == "bigint"
   and ty["avg_int"] == "numeric", str(ty))

# ---------- EY2 活体：面板池子读数 ----------
TRUTH = q("SELECT COALESCE(SUM(amount),0)::bigint FROM pool_donations")
NROW = q("SELECT count(*) FROM pool_donations")
ok("EY2 前置：站免池有 %s 行、真库合计 %s（非 0 才谈得上对账）"
   % (NROW, TRUTH), NROW != "0" and TRUTH != "0", "truth=%s" % TRUTH)
TOK = login("root")
s, r = call("GET", "/admin/economy-dashboard", token=TOK)
d = (r or {}).get("data") or {}
ok("EY2 面板 pool_balance 等于真库合计（P-535：SUM 无 cast ⇒ 解码失败被 "
   "unwrap_or(0) 演成「池子空了」）",
   s == 200 and d.get("pool_balance") == int(TRUTH),
   "http=%s code=%s api=%s 真库=%s"
   % (s, (r or {}).get("code"), d.get("pool_balance"), TRUTH))

# ---------- EY3 活体：阀门空串不许演成 500 ----------
KEY = "economy_max_buffer_gb"
ORIG = q("SELECT value FROM site_settings WHERE name = '%s'" % KEY)
try:
    psql_stdin("UPDATE site_settings SET value = '' WHERE name = '%s';" % KEY)
    s2, r2 = call("GET", "/admin/economy-dashboard", token=TOK)
    d2 = (r2 or {}).get("data") or {}
    ok("EY3 阀门被清成空串 → 面板回 200 且 cap=0（空=未启用是合法状态）",
       s2 == 200 and (r2 or {}).get("code") == 0
       and d2.get("purchase_cap_gb") == 0.0,
       "http=%s code=%s cap=%s" % (s2, (r2 or {}).get("code"),
                                   d2.get("purchase_cap_gb")))
finally:
    psql_stdin("UPDATE site_settings SET value = '%s' WHERE name = '%s';"
               % (ORIG, KEY))
back = q("SELECT value FROM site_settings WHERE name = '%s'" % KEY)
ok("EY3 阀门原值已回读复原（%s → %s）" % (ORIG, back), back == ORIG,
   "orig=%s back=%s" % (ORIG, back))
s3, r3 = call("GET", "/admin/economy-dashboard", token=TOK)
ok("EY3 复原后面板仍读得到同一个池子数（没把闸门自己跑坏）",
   s3 == 200 and ((r3 or {}).get("data") or {}).get("pool_balance")
   == int(TRUTH), "http=%s" % s3)

# ---------- EY4 点名：两处 cast + 站方在途那条 ----------
dash = no_comments(src("apps/api/src/economy_http/dashboard.rs"))
audit = no_comments(src("apps/worker/src/jobs/audit.rs"))
ok("EY4a 面板池子查询带 ::bigint（P-535 本体）",
   "SUM(amount), 0)::bigint FROM " in dash and "pool_donations" in dash,
   "pool_donations 出现 %d 次" % dash.count("pool_donations"))
ok("EY4b cheat_audit 的差额查询带 (::bigint) AS gap",
   "(SUM(delta_up) - SUM(delta_down))::bigint AS gap" in audit,
   "命中=%s" % audit.count("SUM(delta_up) - SUM(delta_down)"))
ok("EY4c 面板两处 DB 错误都不许演成默认值，且空串走 NULLIF 而不是吞错",
   dash.count(".unwrap_or(") == 0 and "NULLIF((SELECT value FROM site_settings"
   in dash, "unwrap_or 仍存 %d 处、NULLIF=%d 处"
   % (dash.count(".unwrap_or("), dash.count("NULLIF(")))
FR = "apps/api/src/repo/community/forum_reward.rs"
fr_bad = "SELECT sum(amount) FROM spark_ledger" in no_comments(src(FR))
print("   [桩] 站方在途 P-456 收回腿回查 = 无 cast 的 SUM(bigint)：%s"
      "（若是，删帖/删主题的收回恒走 owed=0，只 warn 不收回）" % fr_bad)
ok("EY4d 钉桩：forum_reward.granted_by_idem 仍未收口（这条红了 = 对方已修好，"
   "删掉本桩即可）", fr_bad, "文件=%s" % FR)

# ---------- EY5 棘轮：全仓解码侧缺 cast 的命中数 ----------
CALL_RE = re.compile(r"sqlx::query_(?:scalar|as)")
SINK_RE = re.compile(r"(?:Vec<\(?[^>\n]{0,40}i64|:\s*i64\b|Option<i64>|\(i64,"
                     r"|::<_,\s*i64>|f64)")
AGG_RE = re.compile(r"\b(?:SUM|AVG)\s*\(", re.I)
CAST_RE = re.compile(r"::\s*(?:bigint|int8|numeric|float8|double|text|int\b)",
                     re.I)
STR_RE = re.compile(r'r#"(.*?)"#|"((?:\\.|[^"\\])*)"', re.S)
WRITER_RE = re.compile(r"^\s*(?:INSERT|UPDATE|DELETE)", re.I)
hits = []
for dirpath, dirnames, files in os.walk(os.path.join(REPO, "apps")):
    dirnames[:] = [d for d in dirnames
                   if d not in {"target", ".next", "node_modules", "tests"}]
    for fn in sorted(files):
        if not fn.endswith(".rs") or fn.endswith("_test.rs"):
            continue
        p = os.path.join(dirpath, fn)
        text = no_comments(io.open(p, encoding="utf-8", errors="replace")
                           .read().replace("\r\n", "\n"))
        lines = text.split("\n")
        for m in CALL_RE.finditer(text):
            up = text[:m.start()].count("\n")
            if not SINK_RE.search("\n".join(
                    lines[max(0, up - 10):up + 1])):
                continue
            for sm in STR_RE.finditer(text):
                sln = text[:sm.start()].count("\n")
                if not (up - 2 <= sln <= up + 8):
                    continue
                body = sm.group(1) if sm.group(1) is not None else sm.group(2)
                if not AGG_RE.search(body) or WRITER_RE.search(body):
                    continue
                for item in re.split(r",(?![^()]*\))", body):
                    if AGG_RE.search(item) and not CAST_RE.search(item) \
                            and "count(" not in item.lower():
                        hits.append(os.path.relpath(p, REPO).replace("\\", "/"))
uniq = sorted(set(hits))
print("   棘轮命中：行 %d 处 / 文件 %d 个 = %s"
      % (len(hits), len(uniq), uniq))
ok("EY5 棘轮：缺 cast 命中不得多于基线 6 处（6 处已逐条按 PG 类型定性："
   "integer 列的 SUM 本来就是 bigint；voucher_use 解的是外层 "
   "seeding_hourly() 返回的 bigint）", len(hits) <= 6,
   "命中=%d 文件=%s" % (len(hits), uniq))

print()
summary()
