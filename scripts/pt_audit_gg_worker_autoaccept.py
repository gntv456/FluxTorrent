# -*- coding: utf-8 -*-
"""闸门 GG：worker 侧字幕自动验收必须「一笔一事务」（批一百七十八 P-416）。

洞的形状（P-412 同族，只是这次在 worker）：
  `subreq_sweep` 的交稿超时分支先把 `UPDATE ... SET status = 1 ... WHERE status = 4`
  **autocommit** 落库，再逐笔 `spark_award(db, …)` 发赏金；而 `spark_award` 自己开事务、
  把 begin/insert/update 的每一条失败路径都折成「warn + 跳过」。于是发放一失败：
  单子已经是「已自动验收」（下一 tick 不再命中 `status = 4` ⇒ 没有重试路径），
  站内信照发「赏金 N 已按份额分发」。同键域互斥（F4）只挡住了**双发**，挡不住**零发**。
  认领超时分支同样：回池 CAS 先提交，`subreq.abandon` 审计与站内信都是 `let _ =`
  —— 漏一条审计就等于漏计一次弃单，「30 天内 3 次禁认领」靠拖超时即可绕过（P-49
  当年只修了 RETURNING 取错值，没修失败无声）。

活体 A/B（临时触发器把发钱这步真炸掉，WHEN 条件只认我夹具的键前缀 ⇒ 现场窗口几秒、
不影响别的行）：
  旧形状 → status=1、payouts=0、messages=1（账面谎报「已分发」）
  新形状 → 同一处失败：status 仍 4、payouts=0、messages=0（可重试）
  撤掉故障再跑一遍新形状 → status=1、payouts=1、messages=1 ⇒ 下一 tick 会补上
  （残料：夹具 0 / 我的流水 0 / 触发器函数 0 / root 余额 19435 未变）
  诚实记账：新形状那半我用的是**字面 SQL 的事务级重放**，没跑重建后的 worker 进程
  （现网 worker 与 api 同镜像，等站方提交迁移后一起复验）。

判据：
  GG1 两条分支都取事务（`.begin()` ≥2）；
  GG2 CAS `SET status = 1` 的落库目标必须是 `&mut *tx`；
  GG3 发钱走 `spark_award_tx(&mut tx`，旧签名 `spark_award(db,` 残留 0；
  GG4 本文件不得有 `let _ = sqlx::query` 包住 audit_log / messages / spark_ledger 写；
  GG5 承重自证：同一把尺喂旧形状合成码必须报 ≥3 处，喂现文件 0 处；
  GG6 对账不变量（零容忍，现网实测 0）：`status=1 且 paid_at 非空且 bounty>0` 的求字幕
      必须至少有一笔 `subtitle-bounty-pay:{rid}:{uid}` 流水——这就是「已结算却零出账」
      的直接指纹，任何代码路径再撕一次都会红。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_gg_worker_autoaccept.py`
      GG_LIVE=0 跳过 GG6 的只读对账查询。
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

LIVE = os.environ.get("GG_LIVE", "1") != "0"
WORK = "apps/worker/src/jobs/subtitle_flow.rs"

OLD_SHAPE = r"""
    let auto: Vec<(i64, i64)> = sqlx::query_as(
        "UPDATE subtitle_requests SET status = 1, paid_at = now() WHERE "
        "status = 4 RETURNING id, bounty")
    .bind(days.to_string()).fetch_all(db).await?;
    for (rid, bounty) in auto {
        spark_award(db, claimer, bounty, "subtitle_bounty", &idem).await;
        let _ = sqlx::query("INSERT INTO messages (sender_id, receiver_id) "
                            "VALUES (NULL, $1)").bind(owner).execute(db).await;
    }
    let _ = sqlx::query("INSERT INTO audit_log (id, actor_id, action) "
                        "VALUES (nextval('audit_log_id_seq'), $1, "
                        "'subreq.abandon')").bind(uid).execute(db).await;
"""


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def strip(src):
    return "\n".join(
        ln for ln in src.split("\n")
        if not ln.strip().startswith("//") and not ln.strip().startswith("///"))


def violations(text):
    bad = []
    if text.count(".begin()") < 2:
        bad.append("两条分支没有各自的事务")
    i = text.find("UPDATE subtitle_requests SET status = 1")
    if i < 0:
        bad.append("找不到自动验收 CAS")
    else:
        seg = text[i:i + 700]
        m = re.search(r"execute\(\s*([^)]*)\)", seg)
        dest = (m.group(1) if m else "?").strip()
        if "tx" not in dest:
            bad.append("CAS 落 %s" % dest)
    if "spark_award_tx(" not in text:
        bad.append("发钱不走事务版")
    if re.search(r"spark_award\(\s*db", text):
        bad.append("旧 spark_award(db,…) 残留")
    for m in re.finditer(r"let _ =\s*sqlx::query", text):
        seg = text[m.start():m.start() + 300]
        if any(k in seg for k in ("audit_log", "messages", "spark_ledger")):
            bad.append("let _ = 吞审计/信/流水")
    return bad


body = strip(rd(WORK))
v = violations(body)
ok("GG1-GG4 worker 原子性形状齐", v == [], "违例=%s" % (v,))
old_v = violations(OLD_SHAPE)
ok("GG5 承重：旧形状被抓出 ≥3 处", len(old_v) >= 3, "旧违例=%s" % (old_v,))

begins = body.count(".begin()")
cas = re.search(
    r"UPDATE subtitle_requests SET status = 1.*?execute\(&mut \*tx\)",
    body, re.S) is not None
award = "spark_award_tx(&mut tx" in body and not re.search(
    r"spark_award\(\s*db", body)
ok("GG1 逐单事务出现 ≥2 次", begins >= 2, "begin 次数=%d" % begins)
ok("GG2 自动验收 CAS 落 tx", cas, "status=1 的 UPDATE 目标是 &mut *tx")
ok("GG3 发钱走 spark_award_tx", award, "旧签名残留=0")

if LIVE:
    def psql(q):
        r = subprocess.run(
            ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
             "-d", "fluxtorrent", "-q", "-tA"], input=q,
            capture_output=True, text=True, encoding="utf-8", errors="replace")
        err = (r.stderr or "")
        if "ERROR" in err:
            raise RuntimeError("psql: " + err[:300])
        return (r.stdout or "").strip()

    torn = psql(
        "SELECT count(*) FROM subtitle_requests r WHERE r.status = 1 "
        "AND r.paid_at IS NOT NULL AND r.bounty > 0 AND NOT EXISTS "
        "(SELECT 1 FROM spark_ledger l WHERE l.idempotency_key LIKE "
        "'subtitle-bounty-pay:' || r.id || ':%')")
    ok("GG6 已结算零出账的求字幕＝0（对账不变量）", int(torn) == 0,
       "现网=%s" % torn)
else:
    print("[GG6] GG_LIVE=0 跳过对账查询")

summary()
