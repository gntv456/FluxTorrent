# -*- coding: utf-8 -*-
"""闸门 GF：火花「收回/清算」必须走地板原语（批一百七十七 P-415 / P-415b）。

洞的形状（现网实测）：发放原语不防负
  · `checked_balance`（economy_http/spend.rs:20）只防 int64 **溢出**，负数照放；
  · `users.spark_balance` 是 `bigint NOT NULL` 却没有 CHECK 约束（同表
    `publish_credits` 反而有 `>= 0`）；
  · 三处收回点写的是 `let _ = earn_spark(db, uid, -1 / -2 / -(n), …)`。
  ⇒ 版主删一个零余额会员的主题，账面直接变成「会员欠站里 2 火花」，而失败被
    `let _ =` 吞干净。活体 A/B（同一夹具：SQL 造 status=0 的探针会员 + 主题 +
    回帖，root 以版主身份 DELETE /forums/topics/{id}）：
      旧镜像 :8080 → 余额 0 的会员删完变 **-2**（流水 1 条 -2）；余额 1 的变 **-1**
      新二进制 :8099 → 余额 0 的**保持 0、0 条流水**（夹零并 warn）；余额 1 的变 0
                                 且只写 **1 条 -1**（流水==实际扣减，对账不变量不破）

判据：
  GF1 全仓普查 `earn_spark(` 的实参不得为负（并报出调用基数，命中 0 ≠ 基数 0）；
  GF2 三处收回点必须改调 `clawback_spark(`；
  GF3 地板原语本体必须有 `requested.min(balance.max(0))` 的形状、流水写 `-applied`、
      余额写 `spark_balance - applied`（两者必须是同一个数），夹零/部分收回都要 warn；
  GF4 `let _ =` 吞资金原语 ⇒ 白名单台账（只许缩）：现在只剩 farm_actions 一处
      （事务内 + 注释成文「地块已锁且即将删除，重放不可达」）；reports_resolve 的
      罚金已改成「预期内静默、其它 warn」（P-415b），退回 `let _ =` 就红；
  GF5 承重自证：同一把尺喂旧形状合成码必须抓得住；
  GF6 库侧地板：`spark_ledger.balance_after < 0` 必须 0 条；`users.spark_balance < 0`
      的账号数只许 ≤ 基线（现网 25，都是历史墓碑与旧收回留下的账，清账口径待站长定）。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_gf_clawback_floor.py`
      GF_LIVE=0 可跳过库侧两条只读查询。
"""
import io
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import ok, summary  # noqa: E402

LIVE = os.environ.get("GF_LIVE", "1") != "0"
SRC = os.path.join(ROOT, "apps")
EDIT = "apps/api/src/community_http/post_edit.rs"
TOPIC = "apps/api/src/community_http/post_topic_delete.rs"
CLAW = "apps/api/src/economy_http/clawback.rs"
FINE = "apps/api/src/admin_http/reports_resolve.rs"
NEG_BASELINE = 25
NEG_ROW_BASELINE = 15

OLD_SHAPES = {
    "post": r"""
        let _ = earn_spark(
            &state.repo.db,
            author_id,
            -1,
            "forum-post-del",
            &format!("forum-post-del:{pid}"),
        )
        .await;
    """,
    "topic": r"""
        let _ = earn_spark(
            &state.repo.db,
            op_id,
            -2,
            "forum-topic-del",
            &format!("forum-topic-del:{tid}"),
        )
        .await;
    """,
    "reply": r"""
        let _ = earn_spark(
            &state.repo.db,
            uid,
            -(n as i64),
            "forum-topic-del-reply",
            &format!("forum-topic-del-reply:{tid}:{uid}"),
        )
        .await;
    """,
}

# let _ = 吞资金原语的白名单（只许缩）：文件名 → 允许的原语名
WHITELIST = {"farm_actions.rs": ["earn_spark"]}


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def strip(src):
    return "\n".join(
        ln for ln in src.split("\n") if not ln.strip().startswith("//"))


def rs_files():
    out = []
    for dp, _dn, fns in os.walk(SRC):
        for fn in fns:
            if fn.endswith(".rs"):
                out.append(os.path.join(dp, fn))
    return out


def neg_args(text):
    """返回 text 里「earn/spend 原语的负数实参」命中行（按参数行判，
    参数各占一行是本仓 earn_spark 的固定排版）。"""
    hits = []
    for m in re.finditer(r"(earn_spark(?:_tx)?|clawback_spark(?:_tx)?)\s*\(",
                         text):
        span = text[m.start():m.start() + 400]
        for ln in span.split("\n")[1:8]:
            t = ln.strip()
            if re.match(r"^-\s*\(?\s*\w", t) and not t.startswith("--"):
                hits.append(t[:40])
            if ".await" in t:
                break
    return hits


def swallow(text):
    """`let _ = <资金/通知原语>` 的命中数。"""
    return re.findall(r"let _ =\s*(?:crate::economy_http::)?"
                      r"(spend_spark|earn_spark(?:_tx)?|clawback_spark)", text)


files = rs_files()
codes = {p: strip(io.open(p, encoding="utf-8", errors="replace")
                  .read()) for p in files}
all_neg = []
n_earn = 0
for p, c in codes.items():
    n_earn += len(re.findall(r"earn_spark(?:_tx)?\s*\(", c))
    if CLAW in p.replace("\\", "/"):
        continue
    for h in neg_args(c):
        all_neg.append("%s: %s" % (os.path.basename(p), h))
ok("GF1 发放原语不吃负数", all_neg == [],
   "earn_spark 基数=%d，负数实参命中=%s" % (n_earn, all_neg[:3]))

claw_ok = ("clawback_spark(" in strip(rd(EDIT))) and \
          (strip(rd(TOPIC)).count("clawback_spark(") >= 2)
ok("GF2 三处收回点改调 clawback_spark", claw_ok,
   "post_edit=%d topic_delete=%d"
   % (strip(rd(EDIT)).count("clawback_spark("),
      strip(rd(TOPIC)).count("clawback_spark(")))

cw = strip(rd(CLAW))
ok("GF3 地板原语形状齐", "requested.min(balance.max(0))" in cw
   and ".bind(-applied)" in cw
   and re.search(r"spark_balance - \$2", cw) is not None
   and cw.count("tracing::warn!") >= 3, "夹零/部分收回都出声")

ok("GF3b 罚金不再 let _ = 吞", swallow(strip(rd(FINE))) == [],
   "命中=%s" % (swallow(strip(rd(FINE))),))

live_swallow = []
for p, c in codes.items():
    base = os.path.basename(p)
    allow = WHITELIST.get(base, [])
    for m in re.finditer(r"let _ =\s*(?:crate::economy_http::)?"
                         r"(spend_spark|earn_spark(?:_tx)?|clawback_spark)", c):
        if not any(a in m.group(1) for a in allow):
            live_swallow.append("%s:%s" % (base, m.group(1)))
ok("GF4 吞资金写口只剩白名单", live_swallow == [],
   "白名单=%s 其余命中=%s" % (sorted(WHITELIST), live_swallow[:4]))

old_hits = {k: neg_args(v) + swallow(v) for k, v in OLD_SHAPES.items()}
ok("GF5 承重：旧三处形状都被抓",
   all(len(v) >= 2 for v in old_hits.values()),
   "命中数=%s" % {k: len(v) for k, v in old_hits.items()})

if LIVE:
    def psql(q):
        r = subprocess.run(
            ["docker", "exec", "-i", "-e", "ON_ERROR_STOP=1",
             "flux-postgres", "psql", "-U", "flux", "-d", "fluxtorrent",
             "-q", "-tA"], input=q, capture_output=True, text=True,
            encoding="utf-8", errors="replace")
        err = (r.stderr or "")
        if r.returncode != 0 or "ERROR" in err:
            raise RuntimeError("psql: " + err[:300])
        return (r.stdout or "").strip()

    # 历史证据 vs 新增发生：现网已有 15 条 balance_after<0 的流水，全部由旧收回
    # 留下（forum-*-del 收回 + 闸门探针的 user_purge）。清零要等镜像上线，
    # 存量与增量必须分两条判据（批一百八十九纠偏）：原先那个
    # 「部署后一律零容忍」开关把两件事混成一件——现网 15 条
    # 负 balance_after 全是 2026-09-30~10-03 的历史行（17:06 重部署后新增 0），
    # 零容忍读起来像「地板原语没生效」，实际缺的是一次清账数据迁移，而归站方定尺。
    # 判据自己也要对账（2026-10-22 全量回归实测）：GF6a 原先写的是
    # 「近 7 天不许出现 balance_after<0 的行」，可现网那 59 条全是**已经欠着的
    # 账号收到钱**（shop +1000、forum +2 这类入账行，余额从 -15285 涨到 -11285），
    # 一条都不是「扣费写穿零」。用 `balance_after - amount >= 0` 复原扣费前的
    # 余额才知道有没有跨零——全历史这条都是 0，所以地板原语没被写穿，
    # 红的是判据（把入账读成违约），存量归 GT/HJ/DK 那几条清账账本管。
    cross = psql("SELECT count(*) FROM spark_ledger WHERE balance_after < 0 "
                 "AND balance_after - amount >= 0 "
                 "AND created_at > now() - interval '7 days';")
    ok("GF6a 近 7 天没有「扣费跨零」流水（地板原语承重）",
       int(cross) == 0, "现网=%s（>0 才是收回/扣费真写穿了零）" % cross)
    carry = psql("SELECT count(*) FROM spark_ledger WHERE balance_after < 0 "
                 "AND balance_after - amount < 0 "
                 "AND created_at > now() - interval '7 days';")
    print("   [信息] GF6a2 近 7 天负 balance_after 的**入账续写** %s 条"
          "（欠着的人收到钱，不是新违约；清账口径见 GF6b/GF6c）" % carry)
    bad_rows = psql("SELECT count(*) FROM spark_ledger "
                    "WHERE balance_after < 0;")
    ok("GF6c 历史负 balance_after 存量只许 ≤ 基线（待清账，闸门不擅改）",
       int(bad_rows) <= NEG_ROW_BASELINE,
       "现网=%s 基线=%s" % (bad_rows, NEG_ROW_BASELINE))
    negs = psql("SELECT count(*) FROM users WHERE spark_balance < 0;")
    ok("GF6b 负余额账号数只许 ≤ 基线", int(negs) <= NEG_BASELINE,
       "现网=%s 基线=%s（清账口径待站长定）" % (negs, NEG_BASELINE))
else:
    print("[GF6] GF_LIVE=0 跳过库侧两条只读查询")

summary()
