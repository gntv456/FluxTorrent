//! 列表入口读的查看者偏好（H 层 `torrent_http/list_prefs.rs` 的 SQL 唯一居所）。
//! 2026-10-14 下沉：FRAME §2「H 不 import sqlx」——该点查此前写在 H 层文件里，
//! 属批七十三拆分时带过来的新 SQL（母文件计数未降，不适用存量基线登记）。

use super::Repo;

/// `(incl_dead, torrents_per_page)` 主键点查，与同处 `site_settings` 点查同代价。
/// 读失败按「无偏好」处理：偏好不该把列表主路径拖倒——`(1, 0)` 即
/// 「活种 + 每页条数交后端缺省」。
pub async fn viewer_prefs(repo: &Repo, viewer: i64) -> (i32, i32) {
    sqlx::query_as::<_, (i32, i32)>(
        "SELECT incl_dead, torrents_per_page FROM users WHERE id = $1",
    )
    .bind(viewer)
    .fetch_optional(&repo.read_db)
    .await
    .unwrap_or(None)
    .unwrap_or((1, 0))
}
