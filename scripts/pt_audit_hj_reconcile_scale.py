# -*- coding: utf-8 -*-
"""闸门 HJ：对账告警的聚合口径必须与余额重算同一条尺（批一百八十一 P-419）。

洞：`reconcile_diff_alert` 比的是 `sum(spark_ledger)` vs `sum(spark_balance)`，
而权威口径（`jobs/seeding.rs` 的余额重算、`jobs/maintain.rs` 的月度冻结）是
    余额 = balance_baseline.base_spark + sum(spark_ledger 排除 kind='orphan_offset')
两处漏项各对应一笔真实的永久误报：
  ① 漏 `base_spark`：maintain.rs 每月把旧分区流水**冻结进基线**（归档后主表
     不再有那些行），于是每次冻结都让告警按冻结额报错——现网 base_spark 暂时
     全空，所以这颗雷还没炸；一旦第一次冻结落地，diff 就凭空多出一整个月的量。
  ② 漏排除 `orphan_offset`：0266 挂在 root 名下那条 -52,374 全站对冲行，
     个人重算明写不吃，聚合对照却吃 ⇒ root 永久「漂」52,374。
现网实测（2026-10-19）：旧式 diff = **-2,465,156**；同一条 SQL 从 2026-10-05 起
已刷 **86 条 error**。修后 diff = -2,412,782，其中 52,374 正是 ②，余下全部来自
141 个「余额不为零却一条流水都没有」的演示/种子账户（0018 灌的初始余额既没进基线
也没补流水）——那是真实差异，但**清账动作是数据迁移，归站方定尺**，不由闸门擅自改。
旧注释写「执行 0108 清理后消失」也已被实测推翻（0108 之后这 141 个号仍在）。

判据：
  HJ1 告警 SQL 必须同时含 `base_spark` 与 `kind <> 'orphan_offset'`；
  HJ2 必须把两类漂移分开报数（drift_with_ledger / drift_zero_ledger 两个字段）；
  HJ3 同尺对质：reconcile.rs 的聚合口径与 seeding.rs 的个人重算口径必须用同一条尺
      （两处都出现 base_spark + 都排除 orphan_offset），防止只修一处；
  HJ4 承重自证：喂「旧公式」合成码必须报违例；
  HJ5 活体：root 按修后口径不得再漂（②的回归钉）；
  HJ6 活体棘轮：有流水仍漂的账户数 ≤ 1（现网那 1 个是墓碑号 deleted-39 的清算残差），
      只许降不许升——新增一个就是「动账绕过流水」的真嫌疑。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_hj_reconcile_scale.py`
      HJ_LIVE=0 跳过 HJ5/HJ6 的只读查询。
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

LIVE = os.environ.get("HJ_LIVE", "1") != "0"
REC = "apps/worker/src/jobs/reconcile.rs"
SEED = "apps/worker/src/jobs/seeding.rs"
OLD_FORMULA = r"""
    let (ledger_spark, snapshot_spark): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE((SELECT sum(amount) FROM spark_ledger), 0)::bigint, \
         COALESCE((SELECT sum(spark_balance) FROM users), 0)::bigint",
    )
    .fetch_one(db)
    .await?;
"""


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def strip(src):
    out = []
    for ln in src.split("\n"):
        t = ln.strip()
        if t.startswith("//") or t.startswith("///"):
            continue
        out.append(ln)
    return "\n".join(out)


def alert_block(text):
    """取 reconcile_diff_alert 里那条 spark 聚合 SQL。"""
    i = text.find("pub async fn reconcile_diff_alert")
    j = text.find("ledger_up", i)
    return text[i:j if j > i else i + 3000]


def missing_scales(block):
    bad = []
    if "base_spark" not in block:
        bad.append("聚合未加 balance_baseline.base_spark")
    if "orphan_offset" not in block:
        bad.append("聚合未排除 kind='orphan_offset'")
    return bad


rec = strip(rd(REC))
seed = strip(rd(SEED))
blk = alert_block(rec)
bad = missing_scales(blk)
ok("HJ1 告警口径含 base_spark 且排除 orphan_offset", bad == [],
   "缺项=%s" % (bad,))
ok("HJ2 告警自带两类归因字段",
   "drift_with_ledger" in blk and "drift_zero_ledger" in blk,
   "值班读到的该是能动手的数，不是一个要反查的全站差额")
ok("HJ3 与个人重算同尺", "base_spark" in seed
   and "orphan_offset" in seed and "base_spark" in blk
   and "orphan_offset" in blk, "两处都必须吃基线并排除对冲行")
old_bad = missing_scales(strip(OLD_FORMULA))
ok("HJ4 承重：旧公式被本判据抓住", len(old_bad) == 2, "旧缺项=%s" % (old_bad,))

if LIVE:
    def psql(q):
        r = subprocess.run(
            ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
             "-d", "fluxtorrent", "-q", "-tA"], input=q,
            capture_output=True, text=True, encoding="utf-8", errors="replace")
        err = (r.stderr or "")
        if r.returncode != 0 or "ERROR" in err:
            raise RuntimeError("psql: " + err[:300])
        return (r.stdout or "").strip()

    SCALE = ("COALESCE((SELECT base_spark FROM balance_baseline b "
             "WHERE b.user_id = u.id), 0) + COALESCE((SELECT sum(l.amount) "
             "FROM spark_ledger l WHERE l.user_id = u.id "
             "AND l.kind <> 'orphan_offset'), 0)")
    root_drift = psql(
        "SELECT count(*) FROM users u WHERE u.username = 'root' "
        "AND u.spark_balance <> %s" % SCALE)
    ok("HJ5 root 按修后口径不再漂（orphan_offset 回归钉）",
       int(root_drift) == 0, "漂移标记=%s" % root_drift)
    live_drift = int(psql(
        "SELECT count(*) FROM users u WHERE u.spark_balance <> %s "
        "AND EXISTS (SELECT 1 FROM spark_ledger l WHERE l.user_id = u.id "
        "AND l.kind <> 'orphan_offset')" % SCALE))
    ok("HJ6 有流水仍漂的账户数 ≤ 基线（只降不升）", live_drift <= 1,
       "现网=%s 基线=1（那 1 个是 deleted-39 的清算残差）" % live_drift)
else:
    print("[HJ5/HJ6] HJ_LIVE=0 跳过活体两条")

summary()
