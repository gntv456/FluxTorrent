# -*- coding: utf-8 -*-
"""闸门 HS：论坛抽奖奖池的钱必须守恒（批二百零一 P-429）。

洞（修 4b9fc61b 之后同一批发现，本批修 worker）：
`apps/worker/src/jobs/lottery.rs::lottery_settle` 原先只有两支——
  · `picked.is_empty()` ⇒ 把 `winners × prize` 整退楼主；
  · 否则 ⇒ 逐个 `picked` 派彩 `prize`。
**参与人数少于中奖名额时（0 < picked < winners），差额
`(winners - picked) × prize` 谁也不退**。而楼主发帖时是按
`winners × prize` 全额押奖池的（`community_http/topic_create.rs:202`
`spend_spark_tx`，P-399 已把它与悬赏的 kind 分开）。
⇒ 那截钱静默蒸发：押款有流水、派彩有流水，全站
「快照 = 基线 + 流水」的对账视图仍然平，**没有任何人会看见这笔损失**。
现网 `topic_lotteries`/`lottery_entries` 均 0 行 ⇒ 属潜伏，
一旦有人真开抽奖就会踩到。

修法：在同一事务里补一支「不足额退还」，幂等键
`forum-lottery-refund-short:<topic_id>`、kind 仍是 `forum_lottery_refund`
（与整退分支同口径，重跑安全）。

判据：
  HS1 结构：`lottery_settle` 里除「无人参与整退」外必须还有 shortfall 退还支
      （`(winners ... - picked.len()) * prize` + `refund-short` 幂等键）；
  HS2 承重：把该支摘掉的合成旧码必须被同一条判据抓住；
  HS3 kind 纪律：两种退还都写 `forum_lottery_refund`，不许新造 kind
      （P-399 口径：同一笔钱在明细里只能有一个可解释的来源名）；
  HS4 活体账面：现网「奖池押出 − 派彩 − 退还」净额必须为 0（抽奖未被用过
      ⇒ 现在恒 0；一旦有人踩到旧码就会变负 ⇒ 红）；
  HS5 正控：判据取数路径写错时不得恒真（同法在代码里必须能搜到
      `forum_lottery` 这个 kind 字面量）。

用法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_hs_lottery_pool.py
      HS_LIVE=0 跳过 HS4 的现网只读查询。
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

LIVE = os.environ.get("HS_LIVE", "1") != "0"
JOB = "apps/worker/src/jobs/lottery.rs"


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def strip_lines(src):
    out = []
    for ln in src.split("\n"):
        t = ln.strip()
        if t.startswith("//") or t.startswith("///"):
            continue
        out.append(ln)
    return "\n".join(out)


def settle_body(src):
    i = src.find("async fn lottery_settle(")
    j = src.find("\n}\n", i)
    return src[i:j if j > i else len(src)]


def gaps(body):
    bad = []
    if "picked.is_empty()" not in body:
        bad.append("整退分支不在了（口径变了，请同步本闸门）")
    if "refund-short" not in body:
        bad.append("缺「参与不足额」的退还支（P-429 本体）")
    if not re.search(r"winners[\s\S]{0,40}picked\.len\(\)", body):
        bad.append("没有按 winners 与实际派彩人数算差额")
    if body.count("forum_lottery_refund") < 2:
        bad.append("两支退还没有都用 forum_lottery_refund")
    return bad


body = settle_body(strip_lines(rd(JOB)))
g = gaps(body)
ok("HS1 结构：不足额奖池会退回楼主（同事务 + 幂等键）", g == [], "缺项=%s"
   % (g,))
old = body.replace('forum-lottery-refund-short:{topic_id}',
                   'forum-lottery-XXX:{topic_id}')
old = re.sub(r"let short[\s\S]*?tx\.commit\(\)\.await\?;", "", old, count=1)
ok("HS2 承重：摘掉退还支的合成旧码被抓", len(gaps(old)) >= 2,
   "抓到=%s" % (len(gaps(old)),))
kinds = set(re.findall(r"'(forum_lottery[a-z_]*)'", strip_lines(rd(JOB))))
ok("HS3 kind 纪律：只用 forum_lottery / forum_lottery_refund 两名",
   kinds <= {"forum_lottery", "forum_lottery_refund"} and
   "forum_lottery_refund" in kinds, "实见=%s" % (sorted(kinds),))
pos = subprocess.run(["grep", "-c", "forum_lottery",
                      "apps/worker/src/jobs/lottery.rs"], cwd=ROOT,
                     capture_output=True, text=True, errors="replace")
ok("HS5 正控：取数路径确实读到了这份文件（防恒真）",
   (pos.stdout or "0").strip().isdigit() and int(
       (pos.stdout or "0").strip() or 0) > 0,
   "命中行=%s" % (pos.stdout or "").strip())

if LIVE:
    def psql(q):
        r = subprocess.run(
            ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
             "-d", "fluxtorrent", "-q", "-tA", "-c", q],
            capture_output=True, text=True, encoding="utf-8", errors="replace")
        if "ERROR" in (r.stderr or ""):
            raise RuntimeError(r.stderr[:300])
        return (r.stdout or "").strip()

    done = psql(
        "SELECT count(*) FROM topic_lotteries WHERE status IN "
        "('drawn','cancelled')")
    leak = psql(
        "SELECT COALESCE(sum(t.net), 0) FROM (SELECT tl.topic_id, "
        " (SELECT COALESCE(sum(l.amount), 0) FROM spark_ledger l "
        "   WHERE l.kind IN ('forum_lottery','forum_lottery_refund')) + "
        " (SELECT COALESCE(sum(l.amount), 0) FROM spark_ledger l "
        "   WHERE l.ref_type = 'topic' AND l.amount < 0) AS net "
        " FROM topic_lotteries tl WHERE tl.status IN ('drawn','cancelled')) t")
    ok("HS4 活体账面：每张已了结抽奖的钱必须闭环（净额=0）；无样本时点名未覆盖",
       int(done) == 0 or int(leak) == 0,
       "已了结=%s 张，净额合计=%s %s" % (done, leak,
                                    "（0 张 ⇒ 本条对未来生效，现网无样本）"
                                    if done == "0" else ""))
    rows = psql("SELECT (SELECT count(*) FROM topic_lotteries) || '|' || "
                "(SELECT count(*) FROM lottery_entries)")
    ok("HS4b 现网抽奖夹具归零（说明本链尚未被真用过，洞属潜伏）",
       rows == "0|0", "topic_lotteries|lottery_entries=%s" % rows)
else:
    print("[HS4] HS_LIVE=0 跳过现网只读查询")

summary()
