# -*- coding: utf-8 -*-
"""常驻闸门：worker 的后台任务不许静默失败（连接池饥饿一族）。

会员视角看不到这一层，但吃到后果的是会员：
`hr_punish`/`hr_warn_ladder` 没跑 = 该下 H&R 处罚的下完不来；
`sweep_stale_peers` 没跑 = 僵尸做种行还在算收益；
`cheat_enforce`/`class_auto_adjust`/`collusion_check` 没跑 = 处置与降级停摆。
而这些失败**只在 worker 日志里喊一声**，面板上 job 依然显示「本轮结束」。

现网事实（2026-10-20，UTC）：worker 日志 24 小时内 29 行
`pool timed out while waiting for an open connection`，集中在
16:13 / 18:13 / 18:33 三次爆发，每次约 10 条 job 同时失败，
覆盖 collect_milestones / risk_score / backfill_pieces_hash / cheat_enforce /
collusion_check / class_auto_adjust / sweep_stale_peers / lottery_settle /
hr_warn_ladder / task_settle / hr_punish。两次爆发之间同一批 job 又是 0.1s 跑完
⇒ 不是任务本身慢，是**池子被抢光**的那几分钟。

机制（读码 + 数据两头对上）：`jobs/locks.rs::with_lock` 为一条 job 占
**两条**连接——advisory lock 那条整个 job 期间不放（放了锁就泄），业务体再
从池里取一条；而调度器一分钟能同时起跑 15 条 job（同一分钟内
`job_status.last_started_at` 最多 15 行）。16 条池只养得起 8 条并发。
修法：`main.rs` 缺省池 16→32；`locks.rs` 加并发槽位（池的一半），
抢不到槽位就按既有 Skip 语义排队/跳过，而不是把「等连接」变成一次失败。

判据：
  J0 前提+正控：job_status 有货且存在 last_ok=true 的行（表空会伪装成全绿）
  J1 活体健康：不许有任何 job 停在「pool timed out」上（复发即红）
  J2 登记完整：日志里失败过的 job key 必须都在 job_status 有行
     （失败却没登记 = 面板永远绿，这是 P-428 同族）
  J3 算术同源：main.rs 的池缺省 == locks.rs 槽位的 2 倍
     （两处必须成倍数；历史上「改一处忘另一处」在本仓反复出现）
  J4 前提守卫：with_lock 仍是「一条持锁连接 + 业务体自取」的形状
     （形状变了，JOB_SLOT_PER_JOB=2 这个折算就要跟着改）
本闸门只读数不改库、不重启容器；修完要生效需站方重建 worker 镜像。

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_worker_job_health.py
      WORKER_CONTAINER 默认 flux-worker
"""
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import ok, psql_stdin, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MAIN = os.path.join(ROOT, "apps", "worker", "src", "main.rs")
LOCKS = os.path.join(ROOT, "apps", "worker", "src", "jobs", "locks.rs")
CONTAINER = os.environ.get("WORKER_CONTAINER", "flux-worker")
POOL_ERR = "pool timed out"


def psql_rows(sql):
    txt = psql_stdin(sql)
    return [ln for ln in txt.splitlines() if ln.strip()]


def log_lines(since="24h"):
    r = subprocess.run(["docker", "logs", "--since", since, CONTAINER],
                       capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    raw = (r.stdout or "") + (r.stderr or "")
    ansi = re.compile(r"\x1b\[[0-9;]*m")
    return [ansi.sub("", l) for l in raw.splitlines()]


def main():
    # ---------- J0 前提 + 正控 ----------
    total = psql_stdin("SELECT count(*) FROM job_status").strip()
    green = psql_stdin(
        "SELECT count(*) FROM job_status WHERE last_ok").strip()
    ok("J0 前提：job_status 共 %s 行、其中 last_ok=true %s 行（都必须 >0）"
       % (total, green), int(total) > 0 and int(green) > 0,
       "表空或全红时，下面的判据会伪装成通过")

    # ---------- J1 活体健康 ----------
    starved = psql_rows(
        "SELECT job || '=' || left(coalesce(last_result, ''), 40) "
        "FROM job_status "
        "WHERE last_result LIKE '%%%s%%' ORDER BY job" % POOL_ERR)
    ok("J1 没有 job 停在「%s」上（当前 %d 条：%s）"
       % (POOL_ERR, len(starved), starved or "无"), not starved,
       "这一轮的连接池饥饿还没退；后果是该 job 整轮动作没做且不会重试")

    # ---------- J2 日志里失败过的 key 都要在面板登记 ----------
    lines = log_lines()
    hit = [l for l in lines if POOL_ERR in l]
    keys = sorted({m.group(1) for l in hit
                   for m in [re.search(r'key="job:([^"]+)"', l)] if m})
    known = {r.strip() for r in psql_rows("SELECT job FROM job_status")}
    untracked = [k for k in keys if k not in known]
    print("   [读数] %s 小时日志内「%s」%d 行，涉及 job %d 个：%s"
          % (24, POOL_ERR, len(hit), len(keys), ", ".join(keys) or "无"))
    ok("J2 失败过的 job 全部在 job_status 有登记（漏 %s）"
       % (untracked or "无"), not untracked,
       "失败却没登记＝面板永远绿（P-428 同族）")

    # ---------- J3 池与槽位的算术同源 ----------
    pool = re.search(r"const JOB_POOL_DEFAULT: u32 = (\d+);",
                     open(MAIN, encoding="utf-8").read())
    slots = re.search(r"const JOB_SLOTS_MAX: usize = (\d+);",
                      open(LOCKS, encoding="utf-8").read())
    per = re.search(r"const JOB_SLOT_PER_JOB: usize = (\d+);",
                    open(LOCKS, encoding="utf-8").read())
    ok("J3 算术：池 %s == 槽位 %s × 每条 job 占 %s 条连接"
       % (pool and pool.group(1), slots and slots.group(1),
          per and per.group(1)),
       bool(pool and slots and per)
       and int(pool.group(1)) == int(slots.group(1)) * int(per.group(1)),
       "三处数字必须成倍数；只改一处等于把饥饿换个姿势")

    # ---------- J4 折算前提的形状还在 ----------
    lk = open(LOCKS, encoding="utf-8").read()
    shape = ("db.acquire().await" in lk) and ("fut" in lk)
    ok("J4 前提：with_lock 仍是「持锁连接 + 业务体另取一条」的形状（=%s）"
       % shape, shape,
       "形状变了就要重算 JOB_SLOT_PER_JOB，否则 J3 的等式没意义")
    summary()


if __name__ == "__main__":
    main()
