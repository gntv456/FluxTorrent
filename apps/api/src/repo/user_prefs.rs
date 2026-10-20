//! 列表入口读的查看者偏好（H 层 `torrent_http/list_prefs.rs` 的 SQL 唯一居所）。
//! 2026-10-14 下沉：FRAME §2「H 不 import sqlx」——该点查此前写在 H 层文件里，
//! 属批七十三拆分时带过来的新 SQL（母文件计数未降，不适用存量基线登记）。

use super::Repo;

/// `(incl_dead, torrents_per_page, sp_state, incl_bookmarked)` 主键点查，
/// 与同处 `site_settings` 点查同代价（同一行的四个已存在列，不多加往返）。
/// 读失败按「无偏好」处理：偏好不该把列表主路径拖倒——
/// `(1, 0, 0, 0)` 即「活种 + 条数交后端缺省 + 促销/收藏都看全部」。
pub async fn viewer_prefs(repo: &Repo, viewer: i64) -> (i32, i32, i32, i32) {
    sqlx::query_as::<_, (i32, i32, i32, i32)>(
        "SELECT incl_dead, torrents_per_page, sp_state, incl_bookmarked \
         FROM users WHERE id = $1",
    )
    .bind(viewer)
    .fetch_optional(&repo.read_db)
    .await
    .unwrap_or(None)
    .unwrap_or((1, 0, 0, 0))
}

/// 版块主题列表读的偏好：`topics_per_page`（0024，0 = 未设置）。
/// P-449 之前这一列只有写口与回显读它，列表按常量 50 出，「面板填 2 照样
/// 20 条」是纯摆设。读失败按 0 处理（与 `viewer_prefs` 同口径：
/// 偏好不该把列表主路径拖倒）。
pub async fn board_per_page(repo: &Repo, viewer: i64) -> i32 {
    sqlx::query_scalar::<_, i32>(
        "SELECT topics_per_page FROM users WHERE id = $1",
    )
    .bind(viewer)
    .fetch_optional(&repo.read_db)
    .await
    .unwrap_or(None)
    .unwrap_or(0)
}
