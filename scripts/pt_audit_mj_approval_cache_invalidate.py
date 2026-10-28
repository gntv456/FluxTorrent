# -*- coding: utf-8 -*-
"""闸门 MJ：改「审核状态」的写口必须同时失效缓存（P-459，2026-10-28）。

形状：站里列表/详情都读缓存（`bump_list_cache_gen()` 的注释原话——
「列表内容发生变化时调用」；详情侧是 `invalidate_tdetail_cache`）。
同域的先例 `services/review_decide.rs` 审完会失效，而
`admin_http/auto_review_ops.rs:174` 批量把 `approval_status 2 → 0`
（按规则恢复待审）**一个失效调用都没有** ⇒ 后台审核队列与相关角标
要等 TTL 才认账；`torrents/interact.rs` 是同判据点出的第二个（见台账理由）。

本轮没有当场补那行调用，原因写清楚（不是回避）：
`auto_review_ops.rs` 的批量恢复循环拿到的是 `db: &PgPool`，
而 `bump_list_cache_gen(state: &AppState)` 要的是 state —— 补调用要改签名
或换一条以 pool 为入参的失效助手。那是接口形状决定，不适合在闸门轮里顺手改，
故先钉桩，把「谁该改、改成什么形状」留下。

判据（都是文件级，形状可查、不靠记忆）：
  MJ1 正控：合成片段（写 approval_status、零失效）必须被判为违例
  MJ2 基数：扫到的 .rs 必须 >500（掉下限＝ROOT 指错，读数不作数）
  MJ3 台账：写了 `approval_status` 变更、却没调失效函数的文件集合 = 台账
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "apps", "api", "src")

# 写审核状态的形状。**必须要求是 SET 段里的赋值**：
# 第一版写 `UPDATE\s+torrents\s+SET[^;]*approval_status`，而 `[^;]*` 会跨过
# `WHERE` 一路吃到同一文件里下一条语句的 `approval_status = 1` 谓词 ⇒
# `repo/offers.rs`（它 SET 的是 approved_at）与 `torrents/interact.rs`
# 都被误算成写口。**尺子过匹配＝假违例**，所以这里把 SET 段限死在遇到
# WHERE 之前，并要求紧跟等号（谓词写法 `AND approval_status = 1` 不再命中
# 的原因是它前面没有 `UPDATE torrents SET`）。
APPROVE_WRITE = re.compile(
    r"UPDATE\s+torrents\s+SET\s+((?:(?!WHERE)[^;])*?)\bapproval_status\s*=",
    re.I)
INVAL = re.compile(r"bump_list_cache_gen|invalidate_tdetail_cache|"
                   r"invalidate_cache|invalidate_home_cache")
COMMENT_LINE = re.compile(r"^\s*(//|/\*)")

# 台账：实测「SET 里写 approval_status、却没失效」的文件 → 定性理由。
# 上轮标 [未定性] 的两条本轮追完：`repo/offers.rs` 与 `torrents/interact.rs`
# 都是**尺子过匹配的假违例**（它们的 approval_status 只在 WHERE 里），
# 已随上面的正则修好并从台账移除；`upload_revive` 坐实为 P-460。
LEDGER = {
    "admin_http/auto_review_ops.rs":
        "P-459 真漏：2→0 批量恢复不失效，后台审核队列要等 TTL（待补调用，"
        "需先解决 pool→state 的入参形状；先例是 services/review_decide.rs）",
    "publish_http/upload_revive.rs":
        "P-460 真漏：复活整行覆写把 `approval_status = $16` 写回 SET 里"
        "（可写回 1＝上架可见），全链路查无任何失效 ⇒ 症状是「复活成功但"
        "列表里没有」；同属 pool→state 入参形状问题，与 P-459 一起定",
    "torrents/interact.rs":
        "已核：repo 层 `edit_torrent` 把 approval_status 回退成 0（1→0＝下架），"
        "失效在 handler `torrent_http/manage.rs:146` 的 "
        "invalidate_tdetail_cache——而该函数体内（aggregate.rs:65-73）除清详情桶"
        "外还 bump_list_cache_gen ⇒ 两层一起失效，本条**不是漏**",
}


def clean(txt):
    txt = re.sub(r"/\*.*?\*/", " ", txt, flags=re.S)
    return "\n".join(ln for ln in txt.splitlines()
                     if not COMMENT_LINE.match(ln))


files = []
for dp, _d, ns in os.walk(SRC):
    for n in ns:
        if n.endswith(".rs"):
            files.append(os.path.join(dp, n))
ok("MJ2 基数：扫到 >500 个 .rs（掉下限＝尺子没读到代码）",
   len(files) > 500, "文件=%d" % len(files))

flagged = set()
for p in files:
    t = clean(open(p, encoding="utf-8", errors="replace").read())
    if not APPROVE_WRITE.search(t):
        continue
    if INVAL.search(t):
        continue
    rel = os.path.relpath(p, SRC).replace(os.sep, "/")
    if not rel.startswith("sql_guard"):   # 守卫自检用的合成 SQL，不算写口
        flagged.add(rel)

sample = ("sqlx::query(\"UPDATE torrents SET approval_status = 0 "
          "WHERE id = $1\").execute(db).await;")
ok("MJ1 正控：合成「改审核状态但零失效」片段必须被判为违例",
   bool(APPROVE_WRITE.search(sample)) and not INVAL.search(sample),
   "判违例=%s" % (bool(APPROVE_WRITE.search(sample))
                  and not INVAL.search(sample)))

new = flagged - set(LEDGER)
gone = set(LEDGER) - flagged
ok("MJ3 写审核状态却不失效的文件集合 = 台账（新增=新漏口；减少=补了要销号）",
   not new and not gone,
   "新增=%s 已消失=%s 台账=%s"
   % (sorted(new), sorted(gone), sorted(LEDGER)))
for k, why in sorted(LEDGER.items()):
    print("    [台账] %-42s %s" % (k, why))

summary()
