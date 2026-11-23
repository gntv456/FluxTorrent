//! 详情聚合的「个人段」（从 aggregate.rs 拆出，2026-11-23；2026-11-24 去 SQL）。
//!
//! 共享段（torrent/detail/files/nfo/tags）按 (id, reveal) 进 30s Redis 缓存、
//! 跨用户复用；但有几样东西**因人而异**，不能进共享缓存，必须每个请求
//! 按当前用户实时算：
//!
//! - `is_owner` / `purchased` / `bookmarked` / `thanked`：viewer 态布尔
//! - 保种认领位
//! - 首屏 thanks / comments 列表（comments 带点赞 viewer 态，0155）
//!
//! 本层只做**编排**：四个取数并行发起、决定各自的失败语义。SQL 一律不在
//! 这里——FRAME §2「H 不 import sqlx」，取数实现见 `repo::torrent::detail`。
//! 把结果叠回共享段、组装最终响应仍由 `aggregate::assemble` 做。

use crate::errors::DomainResult;
use crate::repo::torrent::detail::{preserve_claim_state, PreserveClaimState};
use crate::state::AppState;
use crate::torrents;

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
    state: &AppState,
    torrent_id: i64,
    uid: i64,
) -> DomainResult<ViewerSegment> {
    let repo = &state.repo;
    let (flags, preserve, thanks, comments) = tokio::join!(
        crate::repo::torrent::detail::viewer_flags(repo, torrent_id, uid),
        preserve_claim_state(repo, torrent_id, uid),
        torrents::list_thanks(&repo.db, torrent_id),
        torrents::list_comments_as(&repo.db, torrent_id, 20, uid),
    );
    let comments = comments?;
    let (is_owner, purchased, bookmarked, thanked) = flags;
    Ok(ViewerSegment {
        is_owner,
        purchased,
        bookmarked,
        thanked,
        preserve,
        thanks: thanks.unwrap_or_default(),
        comments,
    })
}
