#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""ND：`catalog_jobs` 里声明的每个 job 必须有调度落点（P-471 的防复发尺）。

P-471 实况：catalog 写着 `risk_backtest` cadence=24h，但 `run.rs` 的 tick1d
分支里从来没有它，`manual.rs` 之外无任何调用 ⇒ `job_status.last_started_at`
停在 2026-10-16（唯一一次是面板手点），12 天没自己跑过。这类缺陷不会报错、
不会变红，只会「表上写着有节奏、实际上没人跑」——所以需要一把集合尺。

三条判据：
① 声明 ⊆ 调度 ∪ 例外台账：catalog 里的名字必须能在 run.rs 找到调度点
  （`job:NAME` / `NAME(...)`），或在 EXEMPT 里写明「它由哪个专属循环拉起」。
② 调度 ⊆ 声明：run.rs 里出现但 catalog 没声明的 job 也要红——那说明面板
  看不到它、运维不知道它存在。
③ 库里有状态行但 catalog 没声明：同样红（状态行只会由 mark_start 或 catalog
  播种产生，多出来的就是孤儿）。
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

CAT = "apps/worker/src/jobs/catalog_jobs.rs"
RUN = "apps/worker/src/jobs/run.rs"
# 例外台账：不在 run.rs 里调度，但有专属拉起点的 job -> 理由
EXEMPT = {
    "consume_announce": "main.rs 里 spawn 的 Redis 消费循环",
    "consume_agent_blocks": "main.rs 里 spawn 的 Redis 消费循环",
    "spark_merkle": "main.rs:73 spawn(spark_merkle_loop) 专属循环",
}
# 库里有状态行、catalog 没声明（孤儿行）-> 理由
DB_ORPHAN = {
    "consume_xreport": "历史消费循环留下的状态行，catalog 未声明（无害，待清理）",
}
# run.rs 有调度、catalog 未声明 -> 理由（面板看不见 / 不能手点，但确实在跑）
DISPATCH_UNDECLARED = {}  # contract_settle 已于 10-28 补进 catalog_jobs（ND3 由红转绿）

cat = open(CAT, encoding="utf-8", errors="replace").read()
run = open(RUN, encoding="utf-8", errors="replace").read()

declared = set(re.findall(r'name:\s*"([a-z0-9_]+)"', cat))
dispatched = set(re.findall(r'job:([a-z0-9_]+)"', run))
dispatched |= set(re.findall(r'shard_lock!\([^,]+,\s*"job:([a-z0-9_]+)"', run))
dbrows = {ln.strip() for ln in psql_stdin(
    "SELECT job FROM job_status").splitlines() if ln.strip()}

no_runner = sorted(declared - dispatched - set(EXEMPT))
orphan_dispatch = sorted(dispatched - declared
                         - set(DISPATCH_UNDECLARED))
orphan_db = sorted(dbrows - declared - set(DB_ORPHAN))


def missing_runner(decl, disp, exempt):
    return sorted(decl - disp - set(exempt))


ok("ND1 正控：合成件「只声明、不调度」必须被点名",
   missing_runner({"ghost_job"}, set(), {}) == ["ghost_job"], "")
ok("ND1b 正控（反向）：调度点存在或进了例外台账都算过",
   missing_runner({"a"}, {"a"}, {}) == []
   and missing_runner({"b"}, set(), {"b": "x"}) == [], "")
ok("ND2 catalog 声明 ⊆ 调度 ∪ 例外台账（少一个=又一个 risk_backtest）",
   not no_runner, "无落点=%s" % no_runner)
ok("ND3 调度 ⊆ 声明（run.rs 有、catalog 没声明 = 面板看不见的幽灵任务）",
   not orphan_dispatch, "幽灵调度=%s" % orphan_dispatch)
ok("ND4 库里的状态行 ⊆ 声明 ∪ 孤儿台账", not orphan_db,
   "孤儿状态行=%s" % orphan_db)
ok("ND5 反空转：catalog 声明数须 > 50（解析坏会读成空集而假绿）",
   len(declared) > 50, "声明=%d 调度=%d" % (len(declared), len(dispatched)))
print("   [实况] 声明 %d / 调度 %d / 例外 %d / 孤儿状态行 %d"
      % (len(declared), len(dispatched), len(EXEMPT), len(DB_ORPHAN)))
summary()
