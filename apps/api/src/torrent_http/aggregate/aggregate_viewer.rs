//! 详情聚合的「个人段」取数（从 aggregate.rs 拆出，2026-11-23）。
//!
//! 共享段（torrent/detail/files/nfo/tags）按 (id, reveal) 进 30s Redis 缓存、
//! 跨用户复用；但有几样东西**因人而异**，不能进共享缓存，必须每个请求
//! 按当前用户实时算：
//!
//! - `is_owner` / `purchased` / `bookmarked` / `thanked`：viewer 态布尔
//! - 保种认领位（见 `aggregate_preserve`）
//! - 首屏 thanks / comments 列表（comments 带点赞 viewer 态，0155）
//!
//! 本模块只负责**取**，返回一个 `ViewerSegment`；把它叠回共享段、组装
//! 最终响应仍由 `aggregate::assemble` 做。

use sqlx::PgPool;

use crate::errors::DomainResult;
use crate::torrents;

use super::aggregate_preserve::{preserve_state, PreserveClaimState};

/// 一次请求里「与当前用户有关」的全部数据。
pub(super) struct ViewerSegment {
    pub is_owner: bool,
    pub purchased: bool,
    pub bookmarked: bool,
    pub thanked: bool,
    pub preserve: Option<PreserveClaimState>,
    pub thanks: Vec<torrents::ThankRow>,
    pub comments: Vec<torrents::CommentRow>,
}

/// 并行取个人段。comments 读失败向上抛——首屏评论区缺了要报错，
/// 不能静默成「暂无评论」。
pub(super) async fn load_viewer_segment(
    db: &PgPool,
    torrent_id: i64,
    uid: i64,
) -> DomainResult<ViewerSegment> {
    let (own, preserve, thanks, comments) = tokio::join!(
        viewer_flags(db, torrent_id, uid),
        preserve_state(db, torrent_id, uid),
        torrents::list_thanks(db, torrent_id),
        torrents::list_comments_as(db, torrent_id, 20, uid),
    );
    let comments = comments?;
    Ok(ViewerSegment {
        is_owner: own.0,
        purchased: own.1,
        bookmarked: own.2,
        thanked: own.3,
        preserve,
        thanks: thanks.unwrap_or_default(),
        comments,
    })
}

/// 当前用户对这颗种子的四个 viewer 态布尔。
///
/// 共享缓存里存的是「第一个访问者」的答案，必须按当前用户重算
/// （0285 修过一次，被并行批次覆盖回 `let _ = uid;`，症状：B 打开付费种
/// 看到「已购买/我发布」，付费墙被隐藏）。重算失败宁可回落保守值
/// （非本人/未购买），也不能端出缓存里别人的答案——那正是 0285 的症状
/// 会在 DB 抖动时复现的路径。
async fn viewer_flags(
    db: &PgPool,
    torrent_id: i64,
    uid: i64,
) -> (bool, bool, bool, bool) {
    sqlx::query_as::<_, (bool, bool, bool, bool)>(
        "SELECT (t.owner_id = $2) AS is_owner, \
                EXISTS(SELECT 1 FROM torrent_purchases p \
                        WHERE p.torrent_id = t.id \
                          AND p.user_id = $2) AS purchased, \
                EXISTS(SELECT 1 FROM bookmarks b \
                        WHERE b.torrent_id = t.id \
                          AND b.user_id = $2) AS bookmarked, \
                EXISTS(SELECT 1 FROM thanks th \
                        WHERE th.torrent_id = t.id \
                          AND th.user_id = $2) AS thanked \
         FROM torrents t WHERE t.id = $1",
    )
    .bind(torrent_id)
    .bind(uid)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .unwrap_or((false, false, false, false))
}
