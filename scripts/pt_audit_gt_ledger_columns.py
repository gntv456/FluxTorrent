# -*- coding: utf-8 -*-
"""闸门 GT：火花流水表（spark_ledger）的列契约普查。

缘起（批二百三十八 P-479，实测）：`crates/flux-shared/src/lib.rs::earn_spark`
往 `spark_ledger` 插的是 `(user_id, delta, kind, memo, idempotency_key)`——
现网这张表**没有 delta、没有 memo**（净额列叫 `amount`），`id` 无默认值也漏了。
于是 `gift_spark`（商城在售档位 id=10，价 1200 赠 1000）的发货腿结构性失败：
`column "delta" ... does not exist`（42703）⇒ 整笔回滚。
活体双基线（现网旧镜像 vs 侧进程 HEAD+修复）实测：
    旧：POST /shop/buy ⇒ 500「内部错误」，余额 4000→2800（钱扣了），
        gift 流水 0 行，订单卡 fulfill_state='frozen'
    新：200 ok，余额 2800→2600（净 -200），流水恰 1 行 +1000，订单 done
这一族的通用病形是「站方代码里的列名与真实 schema 脱节」——表改名/拆列时
只有运行期第一次调用才会发现，而那种发现往往已经扣完钱了。故本闸门把
**源码里每一条 spark_ledger 的 INSERT 列清单**逐条对现网 schema 核一遍。

判据：
  GT1 现网 schema 取到（列名 + NOT NULL 且无默认值的必填列）；
  GT2 全仓 INSERT INTO spark_ledger 的列清单：列名全在 schema 内，且必填列齐；
  GT3 正控：喂一条「旧形状」的假语句（delta/memo/缺 id），普查必须报错——
      报不出来＝GT2 是恒真判据；
  GT4 事务内 A/B：新 SQL 字面量 BEGIN…ROLLBACK 能落账且 balance_after 对，
      旧 SQL 字面量必须 42703（两侧都在事务里，零残留）；
  GT5 有流水的用户里「余额 ≠ 流水净额」的存量 ≤ 基线（闸门不擅改账）。

跑法：PYTHONIOENCODING=utf-8 python scripts/pt_audit_gt_ledger_columns.py
"""
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

ROOTDIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC_DIRS = [os.path.join(ROOTDIR, "apps", "api", "src"),
            os.path.join(ROOTDIR, "apps", "worker", "src"),
            os.path.join(ROOTDIR, "crates")]
TABLE = "spark_ledger"


def rel(p):
    return os.path.relpath(p, ROOTDIR).replace("\\", "/")


def rs_flat(*parts):
    """读 Rust 源码：剥 // 注释（否则自己刚写的说明会被当成判据对象），
    再把字符串字面量里的 `\\` + 换行拼回去（Rust 的续行写法）。"""
    p = os.path.join(*parts)
    t = open(p, encoding="utf-8", errors="replace").read()
    t = "\n".join(re.sub(r"(?<!:)//.*", "", ln) for ln in t.split("\n"))
    return t.replace("\\\n", "")


def rs_files():
    for d in SRC_DIRS:
        for base, _dirs, names in os.walk(d):
            for n in names:
                if n.endswith(".rs"):
                    yield os.path.join(base, n)


def inserts(text):
    """返回 [(列名集合, 该 INSERT 前 60 字符原文)]。"""
    out = []
    for m in re.finditer(r"INSERT\s+INTO\s+" + TABLE + r"\s*\(([^)]*)\)",
                         text, re.I):
        cols = {c.strip().strip('"') for c in m.group(1).split(",")
                if c.strip()}
        out.append((cols, m.group(0).replace("\n", " ")[:60]))
    return out


def sval(q):
    return psql_stdin(q).strip()


print("== GT1 现网 schema ==")
cols_rows = psql_stdin(
    "SELECT column_name || '#' || is_nullable || '#' || "
    "coalesce(column_default,'') FROM information_schema.columns "
    "WHERE table_name = '%s' ORDER BY ordinal_position;" % TABLE)
schema = {}
for line in cols_rows.splitlines():
    n, nullable, dflt = line.split("#")
    schema[n] = (nullable == "NO" and dflt == "")
ok("GT1 %s 的列取到 %d 个，其中必填（NOT NULL 且无默认值）= %s"
   % (TABLE, len(schema), sorted(k for k, v in schema.items() if v)),
   len(schema) >= 6, psql_stdin(
       "SELECT count(*) FROM information_schema.columns WHERE table_name = "
       "'%s';" % TABLE))

print("== GT2 源码里的 INSERT 列清单逐条对账 ==")
# `spark_ledger.balance_after` 是「这一笔之后余额多少」的逐行链：会员页
# `/me/sparks` 与后台流水都读它（前端对 NULL 显示「—」，不假报 0）。少填一列
# ＝那一行永久断链，事后无法复原（当时的余额没别处记）。现网实测 188 行 NULL
# （seeding_reward 123 / achievement 53 / social_team 12，最晚 10-16）都是历史
# 写入；本轮把**当下仍缺这一列的写口**逐个点名。
stmts, found, bad, noafter = 0, 0, [], []
for f in rs_files():
    t = rs_flat(f)
    stmts += len(re.findall(r"INSERT\s+INTO\s+" + TABLE, t, re.I))
    for cs, frag in inserts(t):
        found += 1
        unknown = sorted(cs - set(schema))
        missing = sorted(k for k, v in schema.items() if v and k not in cs)
        if unknown or missing:
            bad.append((rel(f), sorted(cs), unknown, missing, frag))
        if "balance_after" not in cs:
            noafter.append("%s" % rel(f))
ok("GT2 全仓 %d 条 %s 的 INSERT：列名全在 schema 内、必填列齐（违例 %d）"
   % (found, TABLE, len(bad)), found >= 6 and not bad,
   " | ".join("%s 未知=%s 缺=%s" % (b[0], b[2], b[3]) for b in bad[:3])
   or "全部对齐")
RECOMPUTE_ONLY = set(["apps/worker/src/jobs/preserve.rs",
                      "apps/worker/src/jobs/seeding_referral.rs"])
ok("GT2b 既改余额又写流水的语句必须带 balance_after（违例 %d：%s）"
   % (len(set(noafter) - RECOMPUTE_ONLY),
      sorted(set(noafter) - RECOMPUTE_ONLY)),
   set(noafter) - RECOMPUTE_ONLY == set(),
   "例外=%s 是「只写流水、余额交给 seeding 小时重算」的集合式结算，写流水"
   "那一刻的余额本就未知；例外要成立，得看下一条判据"
   % sorted(RECOMPUTE_ONLY))
lazy_bad = []
for f in sorted(RECOMPUTE_ONLY):
    if re.search(r"UPDATE\s+users", rs_flat(os.path.join(ROOTDIR, f))):
        lazy_bad.append(f)
ok("GT2d 例外清单名副其实：%s 里确实不自己改余额（违例=%s）"
   % (sorted(RECOMPUTE_ONLY), lazy_bad), not lazy_bad,
   "哪天这些文件加了 UPDATE users，就必须同时带 balance_after，"
   "并从例外清单里删掉")
ok("GT2c 普查抓到的语句数 == 原文出现次数（%d vs %d）" % (found, stmts),
   found == stmts, "两者不等＝有 INSERT 的形状没被解析出来，先修解析器再谈对账")

print("== GT3 正控：旧形状必须被普查抓出 ==")
OLD_SHAPE = ('INSERT INTO spark_ledger (user_id, delta, kind, memo, '
             'idempotency_key) VALUES ($1, $2, $3, \'\', $4)')
ctl = inserts(OLD_SHAPE)
ctl_bad = []
if ctl:
    cs = ctl[0][0]
    ctl_bad = sorted(cs - set(schema)) + sorted(
        k for k, v in schema.items() if v and k not in cs)
ok("GT3 正控：把旧语句喂给同一条普查，必须抓到 delta/memo 与漏掉的 id "
   "（实抓=%s）" % ctl_bad,
   set(["delta", "memo", "id"]) <= set(ctl_bad),
   "抓不到＝GT2 是恒真判据")

print("== GT4 事务内 A/B（同字面 SQL，只换那一处）==")
NEW_SQL = ("BEGIN;\nINSERT INTO spark_ledger (id, user_id, amount, kind, "
           "idempotency_key, balance_after) VALUES "
           "(nextval('spark_ledger_id_seq'), 1, 1, 'gt_probe', "
           "'gt-p479-%s', 1);\nSELECT count(*) FROM spark_ledger WHERE "
           "idempotency_key = 'gt-p479-%s';\nROLLBACK;\n" % (os.getpid(),
                                                             os.getpid()))
ok("GT4a 新 SQL 在事务里能落账（回读 1 行）", "1" in psql_stdin(NEW_SQL),
   psql_stdin(NEW_SQL).replace("\n", "|")[:120])
OLD_SQL = ("BEGIN;\nINSERT INTO spark_ledger (user_id, delta, kind, memo, "
           "idempotency_key) VALUES (1, 1, 'gt_probe', '', "
           "'gt-p479-old-%s');\nROLLBACK;\n" % os.getpid())
try:
    psql_stdin(OLD_SQL)
    old_err = ""
except RuntimeError as e:
    old_err = str(e)
ok("GT4b 旧 SQL 必须报错（42703 delta 不存在）——实测：%s"
   % (old_err[:60] or "竟然成功了"), "delta" in old_err, old_err[:160])
gpat = "gt-p479-%s%%" % os.getpid()
left = sval("SELECT count(*) FROM spark_ledger WHERE idempotency_key LIKE "
            "'%s'" % gpat)
ok("GT4c 零残留：本轮探针流水 %s 行（须 0，两侧都在事务里且 ROLLBACK）" % left,
   left == "0", left)

print("== GT5 余额对账（真实不变式：基线 + 流水净额，排除系统对冲行）==")
# 现网余额口径见 `apps/worker/src/jobs/seeding.rs` 的快照重算段：
#   spark_balance == balance_baseline.base_spark
#                     + sum(spark_ledger.amount WHERE kind <> 'orphan_offset')
# 「权威在流水，快照仅展示」；0266 把历史孤儿流水对冲 -52374 记成
# kind='orphan_offset' 的系统行，个人重算**不该**吃它。
# 我第一版把不变式写成「余额 == sum(amount)」，于是 root 被判成不平——
# 实测 52374 + (-103400) = -51026 恰好相等，是把口径读错，不是站点有洞。
# 现网实跑（2026-10-21）：按真实不变式只剩 **39** 一人不平（批一百三十七某次
# 探针直写余额的残留）。root 恰好对平——它靠的是**排除 orphan_offset** 这一项
# （0266 把历史孤儿流水对冲 -52374 挂在 root 名下），不是靠基线表；
# 而本轮 GS/GR 那几个直写注资的探针号（7254/7256~7260）已被 seeding 小时重算
# 刷回「基线+净额」，自己消失了——这正是那条重算段的语义：余额权威在流水。
UNJOURNALLED = set(["39"])
REC = ("WITH l AS (SELECT user_id, sum(amount) FILTER "
       "(WHERE kind <> 'orphan_offset') s FROM spark_ledger GROUP BY user_id) "
       "SELECT string_agg(u.id::text, ',') FROM users u JOIN l "
       "ON l.user_id = u.id LEFT JOIN balance_baseline b "
       "ON b.user_id = u.id WHERE u.spark_balance <> "
       "coalesce(b.base_spark, 0) + coalesce(l.s, 0)")
ids = sval(REC + ";")
now = set(sorted(x for x in ids.split(",") if x))
ok("GT5 余额≠基线+流水净额（排除 orphan_offset）的账号集合 == 在册（实=%s "
   "册=%s）" % (sorted(now, key=int), sorted(UNJOURNALLED, key=int)),
   now == UNJOURNALLED,
   "多出来＝新的直写余额口（先归因再动代码）；少了＝重算已自愈，把该 id "
   "从 UNJOURNALLED 删掉")
ok("GT5b 正控：把「排除 orphan_offset」这一项拆掉后必须报出 root（实=%s）"
   % sorted(
       x for x in sval(REC.replace("(WHERE kind <> 'orphan_offset')",
                                   "(WHERE true)") + ";").split(",")
       if x)[:6],
   "1" in [x for x in sval(REC.replace(
       "(WHERE kind <> 'orphan_offset')", "(WHERE true)") + ";").split(",")],
   "拆掉这一项还报不出 root＝这条 SQL 根本没在对账（不变式写错了）")

summary()
