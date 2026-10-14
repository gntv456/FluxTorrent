//! 列表入口的偏好折算与区间对调（批七十三 P-275，从 `list.rs` 拆出）。
//!
//! 用户控制面板（NP 遗留口径）把 `incl_dead`（0 包括断种 / 1 活种 / 2 断种）
//! 和 `torrents_per_page` 写进 `users` 行：界面能改、能存、能回显，列表读
//! 路径此前一个都不读。实测现网 69 个过审种、暂时无人做种，裸 `/torrents`
//! 恒 0 行，把偏好改成「包括断种」也照样 0 行——偏好是纯摆设。
//!
//! 只折算「URL 参数没给」的那一路：显式 `?alive=` / `?limit=` 优先，
//! 兼容层与 Torznab 不受影响（它们自己把 alive 填成显式值）。

use crate::torrents::TorrentFilter;

// 查看者偏好的点查 SQL 已按 FRAME §2 下沉 `crate::repo::user_prefs`
// （H 层不 import sqlx）：本文件只留「偏好 → 过滤器字段」的折算与口径。

/// `incl_dead` → `alive` 三态。取值就是界面 `<option value>`。
///
/// 1（库默认值）刻意留 `None`：它与后端既有隐式缺省同口径，而
/// `view_scope::shared_cache_safe` 认 `alive.is_none()` 才有首屏共享缓存
/// 资格。填成 `Some(1)` 行为一致却让全部默认账号退化成直查——把 0289
/// 那轮的性能红利丢了。**别顺手把它改成 Some(1)。**
pub(super) fn alive_from_pref(incl_dead: i32) -> Option<i16> {
    match incl_dead {
        0 => Some(0),
        2 => Some(2),
        _ => None,
    }
}

/// `torrents_per_page` → 每页条数。0/负数 = 未设置（建表默认就是 0）。
/// 上限交给 `list_torrents_as` 的 clamp，这里不重复夹。
pub(super) fn limit_from_pref(per_page: i32) -> Option<i64> {
    if per_page > 0 {
        Some(i64::from(per_page))
    } else {
        None
    }
}

/// 上下界颠倒时自动对调（用户先填大后填小很常见，直接判空更友好）。
/// 从 `list.rs` 原样搬来：偏好接线要占行额度，而该文件已在门禁存量上。
pub(super) fn swap_bounds(f: &mut TorrentFilter) {
    if let (Some(a), Some(b)) = (f.size_min, f.size_max) {
        if a > b {
            f.size_min = Some(b);
            f.size_max = Some(a);
        }
    }
    if let (Some(a), Some(b)) = (f.min_seeders, f.max_seeders) {
        if a > b {
            f.min_seeders = Some(b);
            f.max_seeders = Some(a);
        }
    }
    if let (Some(a), Some(b)) = (f.date_from.clone(), f.date_to.clone()) {
        if a > b {
            f.date_from = Some(b);
            f.date_to = Some(a);
        }
    }
    if let (Some(a), Some(b)) = (f.min_leechers, f.max_leechers) {
        if a > b {
            f.min_leechers = Some(b);
            f.max_leechers = Some(a);
        }
    }
    if let (Some(a), Some(b)) = (f.min_completed, f.max_completed) {
        if a > b {
            f.min_completed = Some(b);
            f.max_completed = Some(a);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{alive_from_pref, limit_from_pref};

    /// 纯函数部分：0289 的首屏共享缓存资格依赖「偏好 1 留 None」，
    /// 这条一旦翻转就是性能与串视图双重回归，钉在单元测试里。
    #[test]
    fn pref_folds_to_alive_three_states() {
        assert_eq!(alive_from_pref(0), Some(0), "包括断种");
        assert_eq!(alive_from_pref(2), Some(2), "仅断种");
        assert_eq!(alive_from_pref(1), None, "活种=隐式缺省，留 None 保缓存");
        assert_eq!(alive_from_pref(7), None, "脏值按缺省");
        assert_eq!(limit_from_pref(0), None, "0=未设置");
        assert_eq!(limit_from_pref(-3), None, "负数=未设置");
        assert_eq!(limit_from_pref(30), Some(30));
    }
}
