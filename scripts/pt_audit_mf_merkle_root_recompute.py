# -*- coding: utf-8 -*-
"""闸门 MF：公开树根必须能从 spark_ledger 重算出来（P-455，2026-10-28）。

现网实测（只读）：14 个已封日、1118 枚叶子里 **37 枚指向已不存在的流水行**
（分布在 2026-10-22/23/24/25/26 五天，最重的一天 82 枚里缺 18 枚）。这些用户
**全都还在**（悬空叶子对应的 user 去重后 0 个失踪），而 `apps/api`/`crates` 里
没有任何**删流水**的语句 ——删除来自 **scripts/ 的闸门收尾**：
`e2e_0148_worker_awards.py`、`e2e_0148_workflow.py`、`e2e_promo_buy.py`、
`e2e_round4.py`、`e2e_surface_economy_invariants.py`、`pt_audit_ah_hr.py` 等
都按 `idempotency_key` / `kind` / `user_id` 直删流水。

为什么以前没人发现：站点自己的证明接口（`economy_http/spark_merkle_http.rs`）
是从 `spark_merkle_leaves.leaf/preimage` 出证明的——**验的是自己抄的一份**，
流水被抹掉它测不出来。只有拿 `spark_ledger` 原文重算整棵树才看得见，
而「任何人不信服务端也能自己重算」正是这套凭证的卖点（`crates/flux-shared/src/
merkle.rs` 注释：本模块只证明记录未被事后改动）。流水被删＝**公开树根与账本
脱钩**：游客按 spark_ledger 重算 10-25 那天会得到另一个根。

判据尺子＝按 Rust 实现逐字镜像重算（`flux-ledger-v1|day|id|uid|amount|len(kind):kind
|ref_type|ref_id|balance_after|created`，叶 0x00、内部 0x01，奇数层复制末节点）：
  MF1 正控：至少 N 个完好日重算结果必须与公开树根**逐字相等**（证明镜像没写错）
  MF2 主判据（棘轮）：重算不符的日子集合 = 台账；新出现一天＝又有闸门删了账史
  MF3 台账里的每一缺都要能归因（悬空叶子数 >0），否则要另找原因
"""
import hashlib
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

# 台账：已确证的「流水被闸门收尾删光而树根仍在」的日子（值=悬空叶子数）。
# 这些天永不重算（seal 逻辑 ON CONFLICT DO NOTHING），所以缺只会越来越多；
# 若哪天少了＝有人加了重建能力，要把本条撤掉并重新定性。
LEDGER_BAD = {"2026-10-22": 6, "2026-10-23": 7, "2026-10-24": 3,
              "2026-10-25": 18, "2026-10-26": 3}
SEP = "\t"
DAY_SQL = ("SELECT id, user_id, amount, kind, ref_type, ref_id, "
           "balance_after, to_char(created_at AT TIME ZONE 'UTC', "
           "'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') FROM spark_ledger "
           "WHERE ((created_at AT TIME ZONE 'UTC') + interval '8 hours')"
           "::date = '%s' ORDER BY created_at, user_id, id")


def _dash(v):
    return v if v not in ("", "null") else "-"


def preimage(day, row):
    """逐字镜像 crates/flux-shared/src/merkle.rs::ledger_preimage。"""
    rid, uid, amount, kind, ref_type, ref_id, bal, created = row
    rt = "-" if ref_type in ("", None) else ref_type
    return ("flux-ledger-v1|%s|%s|%s|%s|%d:%s|%s|%s|%s|%s"
            % (day, rid, uid, amount, len(kind), kind, rt,
               _dash(ref_id), _dash(bal), created))


def leaf_hash(pre):
    return hashlib.sha256(b"\x00" + pre.encode()).hexdigest()


def root(leaves):
    if not leaves:
        return leaf_hash("flux-ledger-empty-v1")
    level = [bytes.fromhex(s) for s in leaves]
    while True:
        padded = list(level)
        if len(padded) % 2 == 1 and len(padded) > 1:
            padded.append(padded[-1])
        if len(padded) <= 1:
            return padded[0].hex()
        level = [hashlib.sha256(b"\x01" + a + b).digest()
                 for a, b in zip(padded[0::2], padded[1::2])]


def split(line):
    return [c for c in line.split(SEP)]


days = [ln.split(SEP) for ln in psql_stdin(
    "SELECT day::text, root, leaf_count FROM spark_merkle_days "
    "ORDER BY day;", sep=SEP).splitlines() if ln.strip()]
ok("MF0 前置：sealed 日与树根读得到（读不到＝下面全是空对空）",
   len(days) >= 5 and all(len(d) == 3 for d in days),
   "sealed 日=%d" % len(days))

mismatch, matched, dangling = {}, 0, {}
for day, pub_root, _cnt in days:
    rows = [split(ln) for ln in psql_stdin(DAY_SQL % day, sep=SEP)
            .splitlines() if ln.strip()]
    rows = [r + ["-"] * (8 - len(r)) if len(r) < 8 else r for r in rows]
    lv = [leaf_hash(preimage(day, r)) for r in rows]
    mine = root(lv)
    gone = int(psql_stdin(
        "SELECT count(*) FROM spark_merkle_leaves x WHERE x.day = '%s' "
        "AND NOT EXISTS (SELECT 1 FROM spark_ledger l WHERE l.id = "
        "x.ledger_id);" % day).splitlines()[0].strip())
    if gone:
        dangling[day] = gone
    if mine == pub_root:
        matched += 1
    else:
        mismatch[day] = gone

ok("MF1 正控：重算实现与 Rust 逐字一致（完好日必须能原样复现公开树根）",
   matched >= 5, "复现一致的天=%d/%d" % (matched, len(days)))

bad = set(mismatch) - set(LEDGER_BAD)
healed = set(LEDGER_BAD) - set(mismatch)
ok("MF2 棘轮：重算不符的日子集合 = 台账（新出现一天=又有闸门删了账史）",
   not bad and not healed,
   "新增=%s 已消失=%s 台账=%s" % (sorted(bad), sorted(healed),
                                  sorted(LEDGER_BAD)))

unattrib = {d: g for d, g in mismatch.items() if d in LEDGER_BAD and g == 0}
ok("MF3 归因：台账里每一天都要有悬空叶子（缺而不悬空＝另有原因）",
   not unattrib, "无法归因=%s 悬空合计=%d 只"
   % (unattrib or "-", sum(dangling.values())))
print("[读数] 悬空叶子按天=%s" % sorted(dangling.items()))

# —— MF4 病根棘轮：往 spark_ledger 上写删除语句的脚本数「只减不增」——
# 底数按判据自己的输出登记（2026-10-28 实测 32 支）。逐支改要连着改它们自己的
# 账面断言（删流水本来是为了让「余额==流水净额」对得上），不许半途换；所以这条
# 先把底数钉住：新脚本再学这个习惯＝当场红；某支改掉了＝把上限调低（会点名）。
# 匹配前先剥 `#` 注释行；本文件正文刻意不写那句 SQL 的字面形状（写 SQL 的那段
# 拆成两个半截正则拼起来），免得判据匹配到自己。
DEL_RX = re.compile(r"delete\s+from\s+spark_"
                    r"ledger", re.I)
HERE = os.path.dirname(os.path.abspath(__file__))


def code_only(txt):
    return "\n".join(ln for ln in txt.splitlines()
                     if not ln.lstrip().startswith("#"))


offenders = []
for f in sorted(os.listdir(HERE)):
    if f.endswith(".py") and DEL_RX.search(code_only(
            open(os.path.join(HERE, f), encoding="utf-8",
                 errors="replace").read())):
        offenders.append(f)
LEDGER_DELETE_MAX = 32
gone_hint = ""
if len(offenders) < LEDGER_DELETE_MAX:
    gone_hint = "（已少于上限＝有脚本改掉了，把常量调到 %d）" % len(offenders)
ok("MF4 病根：直删 spark_ledger 的脚本数不得增加（删流水＝改账史，树根会脱钩）",
   len(offenders) <= LEDGER_DELETE_MAX,
   "实测=%d 上限=%d %s" % (len(offenders), LEDGER_DELETE_MAX, gone_hint))
print("[读数] 直删流水的脚本共 %d 支=%s" % (len(offenders), offenders))

summary()
