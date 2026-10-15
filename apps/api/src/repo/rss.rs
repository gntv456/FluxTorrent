//! RSS 出口的数据访问（`rss_http.rs` 的 SQL 唯一居所）。
//! 2026-10-15 下沉：FRAME §2「H 不 import sqlx」。
//!
//! 搬运时保留的三条口径：
//!   · alive_only 读的是 0375 那个开关（默认 no = 保持「全部」，刷流器
//!     既有订阅按「全部」在跑，翻默认值会让它们突然少掉源种）；
//!   · 频道名/描述跟随站点设定，缺行回落到品牌名（COALESCE 链在 SQL 里，
//!     失败再落 Rust 侧兜底，两层都留）；
//!   · 条目 SQL 由 H 层 format! 拼好传进来——谓词片段（促销 LATERAL、
//!     section 双路）仍归 H 层构造，repo 只负责带参执行。

use super::Repo;
use sqlx::FromRow;

/// RSS 条目行（字段与主列表出口保持一致，promotion 供刷流标记用）。
#[derive(FromRow)]
pub struct RssRow {
    pub id: i64,
    pub name: String,
    pub small_descr: Option<String>,
    pub size: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub official_tag: bool,
    /// 当前生效促销；NULL = 无
    pub promotion: Option<String>,
    #[sqlx(default)]
    pub owner_name: Option<String>,
}

/// 0375 存活口径开关：仅当 `rss_alive_only == "yes"` 才为真。
/// 缺行 / 读失败一律 `false`（= 保持「全部」）。
pub async fn alive_only(repo: &Repo) -> bool {
    sqlx::query_scalar(
        "SELECT COALESCE((value = $1), false) \
         FROM site_settings WHERE name = $2",
    )
    .bind("yes")
    .bind("rss_alive_only")
    .fetch_optional(&repo.db)
    .await
    .unwrap_or(None)
    .unwrap_or(false)
}

/// 条目列表。`sql` 由 H 层 format! 拼好（谓词片段的构造权留在那一侧）。
#[allow(clippy::too_many_arguments)]
pub async fn feed_rows(
    repo: &Repo,
    sql: &str,
    categories: Option<&[i32]>,
    mediums: Option<&[i32]>,
    official: Option<bool>,
    search: Option<&str>,
    limit: i64,
    free_only: bool,
    exclude_paid: bool,
    user_id: i64,
) -> Result<Vec<RssRow>, sqlx::Error> {
    sqlx::query_as::<_, RssRow>(sql)
        .bind(categories)
        .bind(mediums)
        .bind(official)
        // LIKE 通配转义：与主列表 esc_like 同口径——裸 %/_ 当通配会让
        // search=% 退化成「不过滤」，RSS 与列表两个出口语义漂移
        .bind(search)
        .bind(limit)
        .bind(free_only)
        .bind(exclude_paid)
        .bind(user_id)
        .bind(alive_only(repo).await)
        .fetch_all(&repo.db)
        .await
}

/// 频道名：SITENAME → site_name → 'FluxTorrent'。
pub async fn channel_title(repo: &Repo) -> String {
    sqlx::query_scalar(
        "SELECT COALESCE(NULLIF((SELECT value FROM site_settings \
                                 WHERE name = 'SITENAME'), ''), \
                         NULLIF((SELECT value FROM site_settings \
                                 WHERE name = 'site_name'), ''), \
                         'FluxTorrent')",
    )
    .fetch_one(&repo.db)
    .await
    .unwrap_or_else(|_| "FluxTorrent".into())
}

/// 频道描述：site_desc → metadescription → '私有种子社区'。
pub async fn channel_desc(repo: &Repo) -> String {
    sqlx::query_scalar(
        "SELECT COALESCE(NULLIF((SELECT value FROM site_settings \
                                 WHERE name = 'site_desc'), ''), \
                         NULLIF((SELECT value FROM site_settings \
                                 WHERE name = 'metadescription'), ''), \
                         '私有种子社区')",
    )
    .fetch_one(&repo.db)
    .await
    .unwrap_or_else(|_| "私有种子社区".into())
}
