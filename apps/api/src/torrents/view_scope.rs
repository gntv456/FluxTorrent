//! 列表首屏缓存的「视图作用域」判定（0289 从 `torrent_http/list.rs` 拆出）。
//!
//! 为什么单独拆：这条判定是**安全边界**而不是性能开关。首屏缓存键
//! `cache:tlist:first:v1:{gen}` 对所有登录用户共用，任何「按查看者视角过滤」的
//! 条件漏在这里，就会把这个人的私有视图在 TTL（45s）内发给别人——
//! 本轮实测就是 `?bookmarked=1` 与 `?approval=2` 两个漏判：
//! 前者让成员互相看到对方的收藏列表，后者把被拒/软删种子写进公共首屏。
//! 条件本身也长到 list.rs 放不进（该文件已在行数门禁存量超限上）。

use super::TorrentFilter;

/// 该请求是否等价于「无筛选默认首屏」——只有它是可跨用户共享缓存的。
/// `cursor_none` 表示没有翻页游标；`limit` 是客户端要求的每页条数（缺省 20）。
pub(crate) fn shared_cache_safe(
    f: &TorrentFilter,
    cursor_none: bool,
    limit: Option<i64>,
) -> bool {
    cursor_none
        && f.category_id.is_none()
        && f.medium_id.is_none()
        && f.grade_id.is_none()
        && f.edition_id.is_none()
        && f.official.is_none()
        && !f.include_dead
        && !f.include_unapproved
        && !f.reverse
        && !f.show_pending
        && !f.show_rejected
        && f.search.is_none()
        && f.sort.is_none()
        && f.tag_ids.is_none()
        && f.sections.is_empty()
        // 状态筛选是用户视角（viewer 的 snatches）
        && f.status.is_none()
        && f.alive.is_none()
        // 0105 高级搜索增强：任一生效即非「首屏等价视图」
        && f.size_min.is_none()
        && f.size_max.is_none()
        && f.date_from.is_none()
        && f.date_to.is_none()
        && f.min_seeders.is_none()
        && f.max_seeders.is_none()
        && f.exclude.is_none()
        && f.promo.is_none()
        && f.owner.is_none()
        && !f.only_mine
        // 0118 补齐项同口径
        && f.min_leechers.is_none()
        && f.max_leechers.is_none()
        && f.min_completed.is_none()
        && f.max_completed.is_none()
        // 0289 实测补：查看者视角的三个筛选
        && !f.bookmarked
        && f.approval.is_none()
        && f.rating_min.is_none()
        && f.anonymous.is_none()
        && limit.unwrap_or(20) == 20
}
