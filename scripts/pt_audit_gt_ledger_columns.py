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
found, bad = 0, []
for f in rs_files():
    for cs, frag in inserts(rs_flat(f)):
        found += 1
        unknown = sorted(cs - set(schema))
        missing = sorted(k for k, v in schema.items() if v and k not in cs)
        if unknown or missing:
            bad.append((os.path.relpath(f, ROOTDIR).replace("\\", "/"),
                        sorted(cs), unknown, missing, frag))
ok("GT2 全仓 %d 条 %s 的 INSERT：列名全在 schema 内、必填列齐（违例 %d）"
   % (found, TABLE, len(bad)), found >= 6 and not bad,
   " | ".join("%s 未知=%s 缺=%s" % (b[0], b[2], b[3]) for b in bad[:3])
   or "全部对齐")

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

print("== GT5 有流水用户的余额对账（按台账集合钉，多一个红、少一个也红）==")
# 在册未记账号（现网实测）：
#   1  root——历年闸门用 `UPDATE users SET spark_balance = <绝对值>` 注资所致
#   39 批一百三十七一次探针的残留
#   7256~7260 2026-10-21 GS/GR 首跑时同样直写 SQL 注资的探针号（已停用）
# 之后 GS/GR 改走 /admin/amountbonus（会落流水），这一族不该再长。
# 清账那天把对应 id 从集合里删掉（少一个也红＝逼着来销桩）。
UNJOURNALLED = set(["1", "39", "7256", "7257", "7258", "7259", "7260"])
ids = sval("WITH l AS (SELECT user_id, sum(amount) s FROM spark_ledger "
           "GROUP BY user_id) SELECT string_agg(u.id::text, ',') FROM users u "
           "JOIN l ON l.user_id = u.id WHERE u.spark_balance <> l.s")
now = set(sorted(x for x in ids.split(",") if x))
ok("GT5 余额≠流水净额的账号集合 == 在册台账（实=%s 册=%s）"
   % (sorted(now, key=int), sorted(UNJOURNALLED, key=int)),
   now == UNJOURNALLED,
   "多出来＝新的直写余额口（先归因再动代码）；少了＝有人清账，请把该 id "
   "从 UNJOURNALLED 删掉")

summary()
