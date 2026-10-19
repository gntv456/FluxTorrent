# -*- coding: utf-8 -*-
"""HP 闸门：做种大赛（contests）的「承诺 vs 实现」在册缺口（批一百七十二）。

现网事实（本轮实测，模块 `module_contests` 关着 ⇒ 会员面 404/4101，不是活故障）：
  · `contests` 有 1 场活跃比赛，描述写着「活动期间做种积分排行前 10 名可获得限定勋章」；
  · `contest_entries` 0 行；`score` 列**全仓没有任何写入者**
    （唯一入口是 `POST /contests/{id}/join`，它只插 (contest_id, user_id)）；
  · 全仓没有任何比赛结算/发奖代码（`apps/worker` 里 contest 只出现在
    `locks.rs` 的作业名清单里，没有对应作业）。
⇒ 一报名：排行榜恒为全 0（列表 SELECT 用 `ORDER BY e.score DESC` 取「榜首」，
   实际是任意一人），承诺的限定勋章**永远不会发**。

这一支不判红（不代站方做功能决策），只做两件事：
  HP1/HP2 静态钉桩：只要「无人计分的 score」与「无人兑付的奖项」两处状态还在，
       就把现网读数打出来并登记在册（新增第三种「承诺」也照样抓）；
  HP3 数据卫生：`is_active` 且已过 `ends_at` 的比赛必须有作业收尾——今天没有作业，
       所以过活跃期不红，只报读数，免得站方误读成「比赛在跑」。

用法：`python -u scripts/pt_audit_hp_contest_promise.py`
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

PLUGINS = "apps/api/src/http/plugins.rs"


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def one(sql):
    r = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-tAc", sql],
        capture_output=True, text=True, encoding="utf-8", errors="replace")
    if r.returncode != 0:
        raise RuntimeError("psql 失败: " + (r.stderr or "")[:300])
    return (r.stdout or "").strip()


def all_rs(pattern):
    """全仓 Rust 源码里匹配到的文件（用于证明「没有写入者」这类否定式结论）。"""
    hits = []
    for base in ("apps/api/src", "apps/worker/src"):
        for dp, _d, fs in os.walk(os.path.join(ROOT, base)):
            for fn in fs:
                if not fn.endswith(".rs"):
                    continue
                t = io.open(os.path.join(dp, fn), encoding="utf-8",
                            errors="replace").read()
                if re.search(pattern, t):
                    hits.append(os.path.relpath(os.path.join(dp, fn), ROOT)
                                .replace("\\", "/"))
    return hits


writers = all_rs(r"UPDATE\s+contest_entries[^;]*score")
issuers = all_rs(r"contest.*(medal|prize|award|发奖|勋章)")
active = one("SELECT count(*) FROM contests WHERE is_active")
overdue = one("SELECT count(*) FROM contests WHERE is_active AND ends_at "
              "< now()")
entries = one("SELECT count(*) FROM contests c JOIN contest_entries e "
              "ON e.contest_id = c.id")
promised = one("SELECT count(*) FROM contests WHERE descr LIKE '%勋章%' "
              "OR descr LIKE '%奖励%' OR descr LIKE '%奖品%'")
print("   [读数] 活跃比赛 %s 场（其中已过活动期 %s 场）、报名行 %s 条、"
      "描述里承诺奖励的 %s 场" % (active, overdue, entries, promised))
print("   [在册] score 写入者=%s；奖项兑付代码=%s"
      % (writers or "无", issuers or "无"))
ok("HP1 钉桩：score 仍无写入者时，排行榜判据不许被当成已实现（读数即可）",
   True, "结构性登记，见上两行；接上计分时把这条改成硬判据")
ok("HP2 钉桩：描述承诺奖励却无兑付代码 ⇒ 不许悄悄上线该模块",
   not (promised != "0" and not issuers and one(
       "SELECT value FROM site_settings WHERE name = 'module_contests'"
   ) == "yes"),
   "开着模块让会员报名一场永远不会发奖的比赛，比不开更糟")
jobs = all_rs(r"jobs/contest|contest_settle|contest_score")
ok("HP3 数据卫生：过活跃期的比赛必须有收尾作业（今天没有 ⇒ 只报读数）",
   True, "活跃中 %s 场 / 已过期 %s 场 / 收尾作业=%s"
   % (active, overdue, jobs or "无"))

summary()
