# -*- coding: utf-8 -*-
"""闸门 BT：worker 任务目录 ↔ 调度派发 ↔ 后台面板 三方对齐（2026-10-25 十一轮）。

为什么要有这支闸门：`GET /admin/jobs` 的面板**唯一**数据源是 `job_status`
（`apps/api/src/admin_p3_http/ops_jobs.rs:51`），而 `job_status` 的行由 worker
启动时的 `sync_job_catalog()` 按代码里的 `JOBS` 目录 upsert，
并且**删除任何不在目录里的行**（`apps/worker/src/jobs/catalog.rs` 尾段）。
`/admin/jobs/run` 的「只有 worker 注册过的任务名可入队」也是查这张表。
⇒ 目录与派发的任何错位都不是显示问题，而是「任务照跑，但面板看不见、
手触被拒」或「面板有个每天都不会跑的按钮」。

实测到的两处错位（十一轮）：
  * `consume_xreport` 在 `run.rs` 里由 `run_consumer()` 每分钟消费，
    但**不在 JOBS 目录里** ⇒ 它现在挂在面板上是因为运行期有人替它写了一行
    `cadence=''`；下次 worker 重启，sync 会把它删掉 ⇒ 面板失去该任务、
    手触回「未知任务」，而代码仍在跑（消费 xreport 的链路没人能看见）。
    空 cadence 还会落进面板 `cadenceLabel` 的回退分支，中文界面出现空分组。
  * `risk_backtest` 在目录里挂着 `cadence="24h"`，但整个 worker 里它只有
    `manual.rs` 的派发分支，`run.rs` 的 tick 分组没有它 ⇒ 面板写着「每天」，
    实际只在人按过的日子跑过（现网读数：`last_started_at=2026-10-16`、
    `last_ok=false`、`last_result=规则集版本不存在：1 / 2`）。

台账口径（不造恒红闸门）：上面两条是**已交回站方定夺的在册债**
（`catalog_jobs.rs` 与 `catalog.rs` 都在站方在途改动里，审站方不代拍目录），
所以判据是「集合只许变少」——出现**新的**错位才红，在册的出声并记现网读数。

  BT1 派发名 ⊆ 目录（新违例即红；在册的进 WARN）
  BT2 目录名 ⊆ 派发 ∪ 手触白名单（新违例即红）
  BT3 手触白名单里的名字必须**真的没有**自动派发（防止反向漂移：
      哪天真接进 tick 了，就该把名字从白名单里摘掉）
  BT4 面板 cadence 值域 ⊆ 六档（空串/自造值即红，白名单除外）
  BT5 现网读证：consume_xreport 此刻仍在跑（last_started_at 两分钟内）
      且仍缺目录登记 ⇒ 若它从 job_status 消失（重启后被 sync 删），这里红
"""
import io
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CATALOG = os.path.join(ROOT, "apps", "worker", "src", "jobs",
                       "catalog_jobs.rs")
CATALOG_OLD = os.path.join(ROOT, "apps", "worker", "src", "jobs", "catalog.rs")
RUN_RS = os.path.join(ROOT, "apps", "worker", "src", "jobs", "run.rs")

# 在册债（交回站方定夺；新增项必须配一句「为什么不修」）
DEBT_NOT_CATALOGUED = {
    "consume_xreport":   "在跑但没进目录：重启后面板会看不见、手触会被拒",
}
DEBT_NO_AUTODISPATCH = {
    "risk_backtest":     "目录挂 24h 但只有手触派发：面板写着「每天」",
}
CADENCE_DOMAIN = {"60s", "10m", "30m", "1h", "6h", "24h"}

FAIL = []
PASS = 0


def ok(name, cond, detail=""):
    global PASS
    if cond:
        PASS += 1
        print("[PASS] %s%s" % (name, ("  " + detail) if detail else ""))
    else:
        FAIL.append(name)
        print("[FAIL] %s  %s" % (name, detail))


def read(path):
    if not os.path.exists(path):
        return ""
    return io.open(path, encoding="utf-8", errors="replace").read()


def strip(t):
    t = re.sub(r"/\*[\s\S]*?\*/", " ", t.replace("\r\n", "\n"))
    return re.sub(r"(?<!:)//[^\n]*", " ", t)


def catalog_names():
    """JOBS 目录里的任务名：优先读拆分后的 catalog_jobs.rs，退回旧单文件。"""
    src = strip(read(CATALOG)) or strip(read(CATALOG_OLD))
    return set(re.findall(r'name:\s*"([a-z0-9_]+)"', src))


def dispatched_names():
    """真的进调度的名字。证据面（十一轮实测口径）：
      ① `run.rs` 里 tick 分组的 `"job:<name>"` 与 `run_consumer(&db, "<name>")`；
      ② `jobs/*.rs` 里**任何** `with_lock(..., "job:<name>", ...)` 体——像
        `spark_merkle` 是 main.rs 直接 spawn 自己的 loop、锁在 merkle.rs 里，
        只看 run.rs 会把它误判成「只有手触」。
      排除：`locks.rs`（那里是 lock 名 → 模块 的映射表，不是派发证据）、
      `manual.rs`（手触分派表）、`catalog*.rs`（目录本身）。
    """
    jobs_dir = os.path.join(ROOT, "apps", "worker", "src", "jobs")
    src = strip(read(RUN_RS))
    names = set(re.findall(r'"job:([a-z0-9_]+)"', src))
    names |= set(re.findall(r'run_consumer\(\s*&db\s*,\s*"([a-z0-9_]+)"', src))
    for fn in sorted(os.listdir(jobs_dir)):
        if not fn.endswith(".rs"):
            continue
        if fn in ("locks.rs", "manual.rs", "catalog.rs", "catalog_jobs.rs"):
            continue
        body = strip(read(os.path.join(jobs_dir, fn)))
        names |= set(re.findall(r'"job:([a-z0-9_]+)"', body))
    return names


def psql(sql):
    r = subprocess.run(
        ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
         "-d", "fluxtorrent", "-v", "ON_ERROR_STOP=1", "-tA", "-F", "|"],
        input=sql, capture_output=True, text=True, encoding="utf-8",
        errors="replace")
    err = (r.stderr or "").strip()
    if r.returncode != 0 or "ERROR" in err:
        raise RuntimeError("psql 失败：%s" % err[:300])
    return [x for x in (r.stdout or "").strip().splitlines() if x.strip()]


def finish():
    print("\n%d 条通过 / %d 条失败" % (PASS, len(FAIL)))
    if FAIL:
        print("失败：" + "、".join(FAIL))
        sys.exit(1)
    sys.exit(0)


def main():
    cat = catalog_names()
    disp = dispatched_names()
    ok("BT0a 目录与派发都读到了名字", bool(cat) and bool(disp),
       "catalog=%d dispatched=%d" % (len(cat), len(disp)))
    if not cat or not disp:
        return finish()

    undis = sorted(disp - cat - set(DEBT_NOT_CATALOGUED))
    ok("BT1 每个派发名都在目录里（新违例即红）", not undis,
       "、".join(undis))
    for n in sorted(disp & set(DEBT_NOT_CATALOGUED)):
        print("[WARN] BT1w 在册债：%s 在跑但不在目录（%s）"
              % (n, DEBT_NOT_CATALOGUED[n]))

    manual_all = set(re.findall(
        r'"([a-z0-9_]+)"\s*=>',
        strip(read(os.path.join(ROOT, "apps", "worker", "src", "jobs",
                                "manual.rs")))))
    no_auto = sorted(n for n in cat
                     if n not in disp and n not in manual_all)
    ok("BT2 目录名要么有自动派发、要么至少有手触分支", not no_auto,
       "、".join(no_auto))
    orphan = sorted(n for n in cat
                    if n not in disp and n in manual_all
                    and n not in DEBT_NO_AUTODISPATCH)
    ok("BT2b 目录挂节奏却只有手触（新错位即红）", not orphan,
       "、".join(orphan))
    for n in sorted(cat & set(DEBT_NO_AUTODISPATCH)):
        print("[WARN] BT2w 在册债：%s（%s）" % (n, DEBT_NO_AUTODISPATCH[n]))

    still_manual = sorted(n for n in DEBT_NO_AUTODISPATCH if n in disp)
    ok("BT3 手触白名单里的名字确实没有自动派发（漂移了就该摘掉）",
       not still_manual, "、".join(still_manual))

    rows = psql("SELECT job || '|' || cadence FROM job_status;")
    live = dict(x.split("|", 1) for x in rows if "|" in x)
    bad_cad = sorted(j for j, c in live.items()
                     if c not in CADENCE_DOMAIN)
    ok("BT4 面板 cadence 值域合法（空串/自造值会落进回退分支）",
       not bad_cad or all(j in DEBT_NOT_CATALOGUED for j in bad_cad),
       "、".join("%s=''" % j if not live[j] else "%s='%s'"
                 % (j, live[j]) for j in bad_cad))

    st = psql("SELECT 'AGE|' || EXTRACT(EPOCH FROM (now() - "
              "last_started_at))::int FROM job_status "
              "WHERE job = 'consume_xreport';")
    age = [x for x in st if x.startswith("AGE|")]
    ok("BT5 现网读证：consume_xreport 仍在跑（≤120s）且行仍在面板里",
       bool(age) and int(age[0].split("|")[1]) <= 120,
       "age=%s" % (age[0].split("|")[1] if age else "行不存在"))
    finish()


main()
