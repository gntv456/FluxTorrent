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

/// `sp_state`（面板「显示促销」八档）→ `promo` 筛选串。
/// 档位取值就是 `usercp-tracker.tsx` 里 `<option value>`：
/// 0 全部 / 1 仅普通 / 2 免费 / 3 2X / 4 2X免 / 5 半折 / 6 2X半 / 7 30%。
/// 串里的 token 与 `?promo=` 白名单同源（`torrent_http/query/promo_norm.rs`），
/// 所以「先解析参数、没给才用偏好」两条走的是同一套匹配实现。
///
/// 4/6 两档必须是**合成档**（两族交集），不能折成 `free,x2` 那种并集：
/// 面板写的是「2X免」，并集却把「只免费」「只 2X」的种也给出去＝给错数据
/// （CZ1g 现网实测：并集 4 个 vs 交集 2 个）。P-291 补的就是这一对 token。
pub(super) fn promo_from_pref(sp_state: i32) -> Option<&'static str> {
    match sp_state {
        1 => Some("none"),
        2 => Some("free"),
        3 => Some("x2"),
        4 => Some("x2free"),
        5 => Some("half"),
        6 => Some("x2half"),
        7 => Some("p30"),
        _ => None,
    }
}

/// `incl_bookmarked`（面板「显示收藏」三档）→ `bookmarked`。
///
/// **1（仅收藏）才接**：0=全部与后端缺省同口径；
/// 2=「仅未收藏」需要一条**排除**维度，`TorrentFilter` 目前只有
/// `bookmarked: bool`（是「只看我收藏的」），把 2 折成 `false` 会演成
/// 「全部」＝给错数据，比不接更坏 ⇒ 刻意留缺省，由 CZ 闸门钉桩 + 现网
/// WARN 记这一档仍未接线（与 `alive_from_pref` 里「1 刻意留 None」同性质）。
pub(super) fn bookmarked_from_pref(incl_bookmarked: i32) -> bool {
    incl_bookmarked == 1
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
    use super::{
        alive_from_pref, bookmarked_from_pref, limit_from_pref,
        promo_from_pref,
    };

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

    /// 面板八档必须逐个折成 `?promo=` 白名单里的合法串（P-289）。
    /// 折错的代价不是「报错」而是「少筛一档还不说」，所以这里断言的是
    /// 每一档的**具体串**，不是「有没有折出东西」。
    #[test]
    fn pref_folds_to_promo_tiers() {
        assert_eq!(promo_from_pref(0), None, "0=全部=不筛");
        assert_eq!(promo_from_pref(1), Some("none"), "1=仅普通");
        assert_eq!(promo_from_pref(2), Some("free"), "2=免费族");
        assert_eq!(promo_from_pref(3), Some("x2"), "3=2X 族");
        assert_eq!(promo_from_pref(4), Some("x2free"), "4=2X 且免");
        assert_eq!(promo_from_pref(5), Some("half"), "5=半折族");
        assert_eq!(promo_from_pref(6), Some("x2half"), "6=2X 且半折");
        assert_eq!(promo_from_pref(7), Some("p30"), "7=30%");
        assert_eq!(promo_from_pref(99), None, "脏值按无偏好");
    }

    /// 收藏三档只接「仅收藏」，「仅未收藏」刻意留缺省（P-290 说明）
    #[test]
    fn bookmark_fold_is_opt_in_only() {
        assert!(!bookmarked_from_pref(0), "0=全部");
        assert!(bookmarked_from_pref(1), "1=仅收藏");
        assert!(!bookmarked_from_pref(2), "2=仅未收藏：无排除维度");
        assert!(!bookmarked_from_pref(7), "脏值按全部");
    }
}
