# -*- coding: utf-8 -*-
"""闸门 GD：论坛悬赏采纳必须「一步失败、整笔回滚」（批一百七十三 P-411）。

盯的形状是本仓反复出现的「状态位先于动账」：
  旧实现 POST /forums/bounty/award 三步各自 autocommit——
  ① CAS `open→awarded` 先落库；② `topic_bounty_awards` 台账用 `let _ =`
  吞失败；③ 最后才 `earn_spark` 给答主。第③步一失败（`?` 上抛），单子已经
  是 awarded，重试撞 CAS ⇒ `LedgerConflict` 永久挡回：楼主发帖时冻住的魔力
  出不去、答主一分没拿、论坛还显示「已采纳」。

活体 A/B（修前/修后同一夹具：预置发放幂等键＝模拟发放失败）：
  · 旧镜像 :8080 → http=200，bounty_status=awarded，答主余额 1→1；撤占位
    重试 400「悬赏已处理」，台账行=1 ⇒ 撕裂确证；
  · 新二进制 :8099 → 400 code=1002「该悬赏的款项已发放过，未重复出账」，
    bounty_status 仍 open；撤占位后 http=200、余额 1→31、台账行=1 ⇒ 回滚干净。

判据（纯静态，不等部署）：
  GD1 采纳口取到事务：`db.begin()` + `tx.commit()`，且通知排在提交之后；
  GD2 三步的落库目标全是 `tx`（CAS / 台账 / 发放）——一处回落 `repo.db`
      就是本缺陷本体；
  GD3 台账不许 `let _ =`（少一条「谁采了谁」＝台账说谎）；
  GD4 发放结果必须判 `SpendOutcome::Spent`（重放不许当成功）；
  GD5 新中文错句在 validation_details.tsv 三列在册；
  GD6 承重自证：同一个检测器喂旧形状合成码必须报出这三处违例，喂现文件
      必须零违例（抓不住旧形状＝判据是橡皮章）。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_gd_bounty_atomic.py`
"""
import io
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import ok, summary  # noqa: E402

TARGET = "apps/api/src/community_http/bounty.rs"
TSV = "apps/api/i18n/validation_details.tsv"
SAY = "该悬赏的款项已发放过，未重复出账"

# 旧形状（修前 HEAD 的字面码，注释另写）——只用于 GD6 承重自证
OLD = r"""
    let n = sqlx::query(
        "UPDATE topics SET bounty_status = 'awarded', bounty_post_id = $2 \
         WHERE id = $1 AND bounty_status = 'open'",
    )
    .bind(tid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    let _ = sqlx::query(
        "INSERT INTO topic_bounty_awards (topic_id, post_id, answerer_id, \
         awarded_by, spark) VALUES ($1, $2, $3, $4, $5)",
    )
    .execute(&state.repo.db)
    .await;
    if spark > 0 {
        let idem = format!("forum-bounty-pay:{}", tid);
        earn_spark(&state.repo.db, answerer, spark, "forum_bounty", &idem)
            .await?;
    }
    notify_user(&state.repo.db, answerer, "comment_reply", "", "").await;
"""


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def strip_comments(src):
    """剥 // 与 /// 行：本仓注释里满是「let _ =」「earn_spark」这些字面词，
    不剥就是自己造红（也就能自己造白）。"""
    return "\n".join(
        ln for ln in src.split("\n")
        if not ln.strip().startswith("//"))


def handler_body(src):
    i = src.index("async fn bounty_award")
    ends = [src.find(m, i) for m in ("\n#[post(", "\n// ----")]
    ends = [e for e in ends if e > i]
    return src[i:min(ends)] if ends else src[i:]


def steps(body):
    """三步 → (步名, 落库目标)。找不到该步时目标记 "?"。"""
    out = []
    for name, anchor in (("CAS 状态翻转", "UPDATE topics SET bounty_status"),
                         ("采纳台账", "INSERT INTO topic_bounty_awards")):
        i = body.find(anchor)
        m = re.search(r"\.execute\(\s*([^)\n]*)", body[i:i + 900]) if i >= 0 \
            else None
        out.append((name, m.group(1).strip() if m else "?"))
    i = body.find("earn_spark")
    m = re.search(r"earn_spark(?:_tx)?\(\s*([^,\n]*)", body[i:i + 200]) \
        if i >= 0 else None
    out.append(("答主发放", m.group(1).strip() if m else "?"))
    return out


def violations(body):
    bad = []
    for name, dest in steps(body):
        if dest == "?" or "repo.db" in dest:
            bad.append(name)
    i = body.find("INSERT INTO topic_bounty_awards")
    if i >= 0 and "let _ =" in body[max(0, i - 120):i]:
        bad.append("台账吞失败")
    j = body.find("earn_spark")
    if j >= 0 and "SpendOutcome::Spent" not in body[j:j + 600]:
        bad.append("发放结果未判")
    return bad


src = strip_comments(rd(TARGET))
body = handler_body(src)
# begin()/commit() 是跨行链式调用（state \n .repo \n .db \n .begin()），按字面
# "db.begin()" 找必然 -1 —— 判据要按调用形状找，不是按我抄的那句字面
mb = re.search(r"\.begin\(\)", body)
mc = re.search(r"\.commit\(\)", body)
begin = mb.start() if mb else -1
commit = mc.start() if mc else -1
notify = body.find("notify_user(")

ok("GD1 采纳口取到事务", begin >= 0 and commit >= 0,
   "begin=%s commit=%s" % (begin, commit))
ok("GD1b 通知排在提交之后（回滚不许发信）",
   0 <= commit < notify, "commit=%s notify=%s" % (commit, notify))
got = [(n, d) for n, d in steps(body)]
ok("GD2 三步全落 tx", all("repo.db" not in d and d != "?" for _, d in got),
   " | ".join("%s=%s" % (n, d) for n, d in got))
ok("GD3 台账不吞失败", "台账吞失败" not in violations(body), "无 let _ =")
ok("GD4 发放判 Spent", "发放结果未判" not in violations(body),
   "SpendOutcome::Spent 在发放步之后 600 字内")
row = [ln.split("\t") for ln in rd(TSV).split("\n")
       if ln.startswith(SAY)]
ok("GD5 新错句三列在册", len(row) == 1 and len(row[0]) == 3,
   "命中 %d 行，列数 %s" % (len(row), len(row[0]) if row else 0))

old_v = violations(strip_comments(OLD))
new_v = violations(body)
ok("GD6 承重：旧形状被抓出三处", len(old_v) >= 3, "旧违例=%s" % (old_v,))
ok("GD6b 现文件零违例（同一把尺）", new_v == [], "新违例=%s" % (new_v,))

summary()
