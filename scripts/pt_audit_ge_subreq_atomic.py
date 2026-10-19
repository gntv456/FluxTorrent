# -*- coding: utf-8 -*-
"""闸门 GE：求字幕「验收/交付」的状态位不得先于动账（批一百七十四 P-412/P-411 家族）。

盯的形状（与 P-411 论坛悬赏同型）：
  · 旧 accept（POST /subtitles/requests/{id}/accept）：CAS `4→1` 先 autocommit，
    再逐个 `earn_spark` 分账——任一处失败这单已是「已验收」而池子里的魔力一分没出，
    重试撞 `status = 4` ⇒ 永久挡回；已发的人拿一半、没发的人永远拿不到。
  · 旧 fulfill（POST /subtitles/requests/{id}/fulfill，subtitle_workflow=0 老通道）：
    同一形状，CAS `0→1` 后整池结算。
  · 两处 free 联动（给种子挂限时免费）都是 `let _ =`：插失败＝承诺的免费凭空消失，
    界面却说结算完成（P-323 家族）。

判据（纯静态，不等部署）：
  GE1/GE2 两条通道的 CAS 必须落 `&mut *tx`，且同函数里有 `.begin()` / `.commit()`；
  GE3 结算核心必须是在事务里出账的 `settle_crew_tx(&mut tx`（旧的 `settle_crew(&db`
      形态一律算违例）；
  GE4 两条通道里不得再有 `let _ =` 包住 promotions 插入；
  GE5 commit 必须排在结算与 free 联动之后、audit 之前；
  GE6 承重自证：同一把尺喂「旧形状」合成码必须报出违例，喂现文件必须零违例；
  GE7 家族在册清单（只许缩不许长）：全站仍把「状态翻转」与「动账」分在两个
      autocommit 里的求字幕口——本批记下 `cancel`（撤销退款循环）与
      `contribute`（追投失败退款 `let _ =`）两只，修掉一只就要从这里划掉一只。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_ge_subreq_atomic.py`
"""
import io
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import ok, summary  # noqa: E402

CLOSE = "apps/api/src/content_http/subtitles_close.rs"
FULFILL = "apps/api/src/content_http/subtitles_fulfill.rs"
CANCEL = "apps/api/src/content_http/subtitles_cancel.rs"
CONTRIB = "apps/api/src/content_http/subtitles_requests.rs"

# 旧形状（修前字面码，注释另写）——只用于 GE6 承重自证
OLD_ACCEPT = r"""
    let row: Option<AcceptRow> = sqlx::query_as(
        "SELECT user_id, bounty, COALESCE(claimed_by, user_id), crew FROM \
         subtitle_requests WHERE id = $1 AND status = 4",
    )
    .bind(rid)
    .fetch_optional(&state.repo.db)
    .await?;
    let n = sqlx::query("UPDATE subtitle_requests SET status = 1, paid_at = \
         now() WHERE id = $1 AND status = 4")
    .bind(rid)
    .execute(&state.repo.db)
    .await?
    .rows_affected();
    let mut paid = settle_crew(&state.repo.db, rid, bounty, claimer, &crew)
        .await?;
    let _ = sqlx::query("INSERT INTO promotions (scope, torrent_id, kind) \
         VALUES ('torrent', $1, 'free', 'subtitle')")
    .execute(&state.repo.db)
    .await;
"""
OLD_FULFILL = r"""
    let row: Option<FulfillRow> = sqlx::query_as::<_, FulfillRow>(
        "UPDATE subtitle_requests SET status = 1, paid_at = now() WHERE id \
         = $1 AND status = 0 RETURNING bounty, crew")
    .bind(rid)
    .fetch_optional(&state.repo.db)
    .await?;
    let mut paid = super::subtitles_close::settle_crew(
        &state.repo.db, rid, bounty, auth.id, &crew).await?;
"""


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def strip(src):
    """剥 // 与 /// 行：注释里满是 `let _ =`、`earn_spark` 这些字面词，
    不剥就是判据自己造红（也就能自己造白）。"""
    return "\n".join(
        ln for ln in src.split("\n") if not ln.strip().startswith("//"))


def body_of(src, marker):
    i = src.index(marker)
    ends = [src.find(m, i) for m in ("\n#[post(", "\n/// ", "\nasync fn",
                                     "\npub(super) async fn")]
    ends = [e for e in ends if e > i]
    return src[i:min(ends)] if ends else src[i:]


def violations(body, cas):
    bad = []
    if ".begin()" not in body:
        bad.append("无事务")
    i = body.find(cas)
    if i < 0:
        bad.append("找不到 CAS")
    else:
        seg = body[i:i + 900]
        m = re.search(
            r"fetch_optional\(\s*([^)]*)\)|execute\(\s*([^)]*)\)", seg)
        dest = (m and (m.group(1) or m.group(2)) or "?").strip()
        if "tx" not in dest:
            bad.append("CAS 落 %s" % dest)
    if "settle_crew_tx(" not in body or "&mut tx" not in body:
        bad.append("结算未走 tx")
    j = body.find("settle_crew_tx(")
    k = body.find(".commit()")
    if k < 0 or (j >= 0 and k < j):
        bad.append("commit 位置不对")
    if re.search(r"let _ =\s*sqlx::query", body):
        bad.append("吞失败写库")
    if re.search(r"settle_crew\(&", body):
        bad.append("旧 settle_crew 调用")
    return bad


cs = strip(rd(CLOSE))
fs = strip(rd(FULFILL))
acc = body_of(cs, "pub(super) async fn subtitle_request_accept")
ful = body_of(fs, "pub(super) async fn subtitle_request_fulfill")
va = violations(acc, "UPDATE subtitle_requests SET status = 1")
vf = violations(fs, "UPDATE subtitle_requests SET status = 1")

ok("GE1 accept CAS 落 tx", "CAS" not in " ".join(va) and ".begin()" in acc,
   "违例=%s" % (va,))
ok("GE2 fulfill CAS 落 tx", ".begin()" in ful and "CAS 落" not in " ".join(vf),
   "违例=%s" % (vf,))
ok("GE3 两条通道都用 settle_crew_tx(&mut tx",
   "settle_crew_tx(" in acc and "settle_crew_tx(" in ful
   and not re.search(r"settle_crew\(&", cs + fs), "旧签名残留=0")
ok("GE4 free 联动不再 let _ =", "吞失败写库" not in va and "吞失败写库" not in vf,
   "accept/fulfill 的 promotions 插入都判 map_err")
ok("GE5 commit 排在结算之后", "commit 位置不对" not in va + vf,
   "两处都先结算后提交")
oa = violations(OLD_ACCEPT, "UPDATE subtitle_requests SET status = 1")
of = violations(OLD_FULFILL, "UPDATE subtitle_requests SET status = 1")
ok("GE6 承重：旧形状被抓", len(oa) >= 3 and len(of) >= 2,
   "旧 accept=%s 旧 fulfill=%s" % (oa, of))
ok("GE6b 现文件零违例（同一把尺）", va == [] and vf == [],
   "accept=%s fulfill=%s" % (va, vf))

# 家族在册清单：仍把「翻状态」与「动账」拆在两个 autocommit 里的求字幕口
INregister = {"cancel": CANCEL, "contribute": CONTRIB}
still = []
for name, rel in sorted(INregister.items()):
    b = rd(rel)
    if "SET status = 2" in b or "let _ = crate::economy_http::earn_spark" in b:
        still.append(name)
ok("GE7 在册清单未长大", sorted(still) == sorted(INregister.keys()),
   "在册=%s（修掉一只就要从这里划掉一只）" % (still,))

summary()
