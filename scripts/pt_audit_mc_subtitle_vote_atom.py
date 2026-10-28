# -*- coding: utf-8 -*-
"""MC：字幕评分的「两条写」必须同生同死（P-452，2026-10-28）。

缺陷本体（读码 + 与同文件 P-131 先例对照）：`subtitle_vote` 原先三条语句各跑
各的——① `SELECT EXISTS(...)` 走连接池、② `INSERT INTO subtitle_votes` **立即
提交**、③ `UPDATE subtitles SET rating_sum/rating_count` 再单独提交。第 ③ 步
失败（连接抖动、进程被杀）就留下**票已记、聚合仍为 0**的半套：详情页显示
「暂无评分」，而月度评选的 `rating_count > 0` 会把这条字幕整条筛掉——只有
下一个人再投一次才自愈。同文件的 `subtitle_report` 早已按 P-131 收进一把事务，
这条是同族漏网。

判据尺子取「函数体切片 + 写入是否都落在 tx 上」；正控用**合成旧形状**（不拿
`git show HEAD:`——修复一提交，HEAD 就与工作树同形，那条正控会永不可能再红）。

  MC1 正控：合成的改前形状必须被同一把尺点名
  MC1b 基数：尺子真的读到函数体与两条写（读空不算合规）
  MC2 工作树：两条写都在 begin..commit 之间，且事务外不再碰连接池
  MC3 存在性判定必须在事务里并锁行（`FOR UPDATE`）——先判后写之间被软删/驳回
      的字幕不能再收票
  MC4 普查：`INSERT INTO subtitle_votes` 全仓只有一处（防第二份实现漂移口径）
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import ok, summary  # noqa: E402

REL = "apps/api/src/content_http/subtitles_govern.rs"
FN = "async fn subtitle_vote("
WRITE1 = "INSERT INTO subtitle_votes"
WRITE2 = "UPDATE subtitles s SET rating_sum"


def blob(rev):
    """取某版本的文件原文（保留给以后要拿真旧码做对照时用）。"""
    r = subprocess.run(["git", "show", "%s:%s" % (rev, REL)],
                       capture_output=True, text=True, encoding="utf-8",
                       errors="replace")
    return r.stdout if r.returncode == 0 else ""


def worktree():
    p = os.path.join(os.path.dirname(os.path.dirname(
        os.path.abspath(__file__))), REL.replace("/", os.sep))
    return open(p, encoding="utf-8", errors="replace").read()


def fn_body(src):
    """从函数签名切到顶格 `}`（Rust 顶层 fn 的收尾花括号在第 0 列）。"""
    i = src.find(FN)
    if i < 0:
        return ""
    j = src.find("\n}\n", i)
    return src[i:] if j < 0 else src[i:j + 3]


def offending(body):
    """返回违例说明；None=这一版是合规形状。"""
    if not body:
        return "函数体取不到（签名搬家了？先修尺子）"
    b = body.find(".begin()")
    c = body.find(".commit()")
    if b < 0 or c < 0 or c < b:
        return "整条链路没有一把事务（begin/commit 缺或倒序）"
    bad = []
    for tag, needle in (("投票写", WRITE1), ("聚合重算", WRITE2)):
        k = body.find(needle)
        if k < 0:
            bad.append("%s 语句不在这个函数里了（口径搬家要同步本闸门）" % tag)
        elif not (b < k < c):
            bad.append("%s 落在事务之外" % tag)
    # 事务窗口内不许再有「直接对连接池发起」的调用（准入门那段只读在 begin
    # 之前，属合法）——这条把「写被拆回池子上」的形状钉死。
    pool = body.find("&state.repo.db", b)
    if 0 <= pool < c:
        bad.append("事务内仍直接对连接池发起调用（位置 %d）" % pool)
    return "；".join(bad) if bad else None


WT_BODY = fn_body(worktree())

# 正控用**合成旧形状**，不拿 `git show HEAD:` ——这条修复一旦提交，HEAD 就与
# 工作树同形，「旧版必须红」会退化成永不可能成立的断言（本仓踩过这个坑：
# 正控钉在会前进的参照物上＝假承重）。合成体逐字抄自改前那三条语句的写法。
OLD_SHAPE = """
async fn subtitle_vote(req: HttpRequest, state: web::Data<Arc<AppState>>) {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM subtitles WHERE id = $1 AND deleted_at \\
         IS NULL AND status = 1)")
        .bind(sid).fetch_one(&state.repo.db).await.unwrap_or(false);
    if !exists { return Err(DomainError::NotFound(sid)); }
    sqlx::query("INSERT INTO subtitle_votes (subtitle_id, user_id, score) \\
         VALUES ($1, $2, $3)")
        .bind(sid).bind(auth.id).bind(body.score)
        .execute(&state.repo.db).await?;
    sqlx::query("UPDATE subtitles s SET rating_sum = v.sum, rating_count = \\
         v.cnt FROM (...) v WHERE s.id = $1")
        .bind(sid).execute(&state.repo.db).await?;
}
"""
head_bad = offending(OLD_SHAPE)
ok("MC1 正控：合成旧形状（三条语句各跑各的）必须被同一把尺点名",
   head_bad is not None and "事务" in head_bad,
   "实测=%s" % (head_bad or "绿了＝尺子不承重"))
ok("MC1b 基数：这一把尺真的读到了两条写与函数体（读空≠合规）",
   len(WT_BODY) > 400 and WRITE1 in WT_BODY and WRITE2 in WT_BODY,
   "函数体=%d 字 两条写都在=%s" % (
       len(WT_BODY), WRITE1 in WT_BODY and WRITE2 in WT_BODY))

wt_bad = offending(WT_BODY)
ok("MC2 工作树：投票与聚合重算同在一把事务内（半套＝票记了、评分仍显示没有）",
   wt_bad is None, "实测=%s" % (wt_bad or "-"))

ok("MC3 存在性判定在事务内并锁行（先判后写之间被软删的字幕不能再收票）",
   "FOR UPDATE" in WT_BODY
   and WT_BODY.find("deleted_at IS NULL") > WT_BODY.find(".begin()"),
   "实测=FOR UPDATE %s、存在性读点在 begin 之后 %s"
   % ("FOR UPDATE" in WT_BODY,
      WT_BODY.find("deleted_at IS NULL") > WT_BODY.find(".begin()")))

SRC = worktree()
n = SRC.count(WRITE1)
ok("MC4 普查：全文件只有一处写 subtitle_votes（多一处=第二份实现要并口径）",
   n == 1, "实测=%d 处" % n)

summary()
