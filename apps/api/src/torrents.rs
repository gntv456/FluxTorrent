//! 种子域仓储与查询（M02/M03）：游标分页 + 覆盖索引（§6.2）。

use serde::Serialize;
use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TorrentRow {
    pub id: i64,
    pub info_hash: String,
    pub name: String,
    pub small_descr: Option<String>,
    pub category_id: i32,
    pub medium_id: i32,
    pub grade_id: Option<i32>,
    pub edition_id: Option<i32>,
    pub size: i64,
    pub seeders: i32,
    pub leechers: i32,
    pub times_completed: i32,
    pub comments: i64,
    #[serde(rename = "official")]
    pub official_tag: bool,
    pub anonymous: bool,
    pub approval_status: i16,
    pub sticky: bool,
    pub owner_name: Option<String>,
    pub promotion: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 详情页扩展字段（简介/文件数/感谢数；列表不需要，独立查询）
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TorrentDetailRow {
    pub id: i64,
    pub descr: Option<String>,
    pub numfiles: i32,
    pub thanks_count: i64,
    pub bookmark_count: i64,
    pub last_action: Option<chrono::DateTime<chrono::Utc>>,
}

/// 详情页文件列表（files 表；无记录时前端隐藏该区块）
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct FileRow {
    pub file_index: i32,
    pub path: String,
    pub size: i64,
}

/// 感谢者列表（近 50 人）
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ThankRow {
    pub username: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Default, Serialize)]
pub struct TorrentFilter {
    pub category_id: Option<i32>,
    pub medium_id: Option<i32>,
    pub grade_id: Option<i32>,
    pub edition_id: Option<i32>,
    pub official: Option<bool>,
    pub include_dead: bool,
    pub search: Option<String>,
    /// 列表排序（旧站 torrents.php 口径）：created（默认）/ seeders / size / completed
    pub sort: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TorrentPage {
    pub items: Vec<TorrentRow>,
    pub next_cursor: Option<String>,
    pub total_estimate: i64,
}

const MAX_LIMIT: i64 = 50;

pub async fn list_torrents(
    db: &PgPool,
    filter: &TorrentFilter,
    cursor: Option<i64>,
    limit: i64,
) -> DomainResult<TorrentPage> {
    let limit = limit.clamp(1, MAX_LIMIT);
    let pattern = filter.search.as_deref().map(|s| {
        format!(
            "%{}%",
            s.replace('\\', "").replace('%', "\\%").replace('_', "\\_")
        )
    });

    // 排序白名单（防注入）；非 id 排序时退化为 OFFSET 无关的「前 N 截断」：
    // 排序键 + id 组成稳定排序，游标仍按 id 翻页（与默认排序一致，简单可靠）。
    let order = match filter.sort.as_deref() {
        Some("seeders") => "t.sticky DESC, t.seeders DESC, t.id DESC",
        Some("size") => "t.sticky DESC, t.size DESC, t.id DESC",
        Some("completed") => "t.sticky DESC, t.times_completed DESC, t.id DESC",
        _ => "t.sticky DESC, t.id DESC",
    };
    let sql = format!(
        r#"
        SELECT t.id, t.info_hash, t.name, t.small_descr, t.category_id, t.medium_id,
               t.grade_id, t.edition_id, t.size, t.seeders, t.leechers, t.times_completed,
               (SELECT count(*) FROM comments c WHERE c.torrent_id = t.id) AS comments,
               t.official_tag, t.anonymous, t.approval_status, t.sticky,
               CASE WHEN t.anonymous THEN NULL ELSE u.username END AS owner_name,
               (SELECT kind::text FROM promotions p
                  WHERE p.torrent_id = t.id AND p.starts_at <= now() AND p.ends_at > now()
                  ORDER BY p.id DESC LIMIT 1) AS promotion,
               t.created_at
        FROM torrents t
        LEFT JOIN users u ON u.id = t.owner_id
        WHERE t.approval_status = 1
          AND ($1::int IS NULL OR t.category_id = $1)
          AND ($2::int IS NULL OR t.medium_id = $2)
          AND ($3::int IS NULL OR t.grade_id = $3)
          AND ($4::int IS NULL OR t.edition_id = $4)
          AND ($5::bool IS NULL OR t.official_tag = $5)
          AND ($6::bool OR t.seeders > 0)
          AND ($7::text IS NULL OR t.name ILIKE $7 ESCAPE chr(92)
               OR t.small_descr ILIKE $7 ESCAPE chr(92)
               OR t.descr ILIKE $7 ESCAPE chr(92)
               OR t.id IN (SELECT torrent_id FROM files WHERE path ILIKE $7 ESCAPE chr(92)))
          AND ($8::bigint IS NULL OR t.id < $8)
        ORDER BY {order}
        LIMIT $9
        "#
    );
    let rows = sqlx::query_as::<_, TorrentRow>(&sql)
        .bind(filter.category_id)
        .bind(filter.medium_id)
        .bind(filter.grade_id)
        .bind(filter.edition_id)
        .bind(filter.official)
        .bind(filter.include_dead)
        .bind(&pattern)
        .bind(cursor)
        .bind(limit + 1)
        .fetch_all(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents t WHERE t.approval_status = 1 \
         AND ($1::int IS NULL OR t.category_id = $1) AND ($2::int IS NULL OR t.medium_id = $2) \
         AND ($3::int IS NULL OR t.grade_id = $3) AND ($4::int IS NULL OR t.edition_id = $4) \
         AND ($5::bool IS NULL OR t.official_tag = $5) AND ($6::bool OR t.seeders > 0) \
         AND ($7::text IS NULL OR t.name ILIKE $7 ESCAPE chr(92) OR t.small_descr ILIKE $7 ESCAPE chr(92) \
          OR t.descr ILIKE $7 ESCAPE chr(92) \
          OR t.id IN (SELECT torrent_id FROM files WHERE path ILIKE $7 ESCAPE chr(92)))",
    )
    .bind(filter.category_id)
    .bind(filter.medium_id)
    .bind(filter.grade_id)
    .bind(filter.edition_id)
    .bind(filter.official)
    .bind(filter.include_dead)
    .bind(&pattern)
    .fetch_one(db)
    .await
    .unwrap_or(0);

    let has_more = rows.len() as i64 > limit;
    let items = rows.into_iter().take(limit as usize).collect::<Vec<_>>();
    let next_cursor = has_more.then(|| items.last().map(|r| r.id.to_string()).unwrap_or_default());
    Ok(TorrentPage {
        items,
        next_cursor,
        total_estimate: total,
    })
}

pub async fn get_torrent(db: &PgPool, id: i64) -> DomainResult<TorrentRow> {
    let page = sqlx::query_as::<_, TorrentRow>(
        r#"
        SELECT t.id, t.info_hash, t.name, t.small_descr, t.category_id, t.medium_id,
               t.grade_id, t.edition_id, t.size, t.seeders, t.leechers, t.times_completed,
               (SELECT count(*) FROM comments c WHERE c.torrent_id = t.id) AS comments,
               t.official_tag, t.anonymous, t.approval_status, t.sticky,
               CASE WHEN t.anonymous THEN NULL ELSE u.username END AS owner_name,
               (SELECT kind::text FROM promotions p
                  WHERE p.torrent_id = t.id AND p.starts_at <= now() AND p.ends_at > now()
                  ORDER BY p.id DESC LIMIT 1) AS promotion,
               t.created_at
        FROM torrents t LEFT JOIN users u ON u.id = t.owner_id
        WHERE t.id = $1 AND t.approval_status = 1
        "#,
    )
    .bind(id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    items_or_not_found(page, id)
}

fn items_or_not_found(mut v: Vec<TorrentRow>, id: i64) -> DomainResult<TorrentRow> {
    if v.is_empty() {
        Err(DomainError::NotFound(id))
    } else {
        Ok(v.remove(0))
    }
}

pub async fn get_torrent_detail(db: &PgPool, id: i64) -> DomainResult<TorrentDetailRow> {
    let row = sqlx::query_as::<_, TorrentDetailRow>(
        r#"
        SELECT t.id, t.descr, t.numfiles,
               (SELECT count(*) FROM thanks th WHERE th.torrent_id = t.id) AS thanks_count,
               (SELECT count(*) FROM bookmarks b WHERE b.torrent_id = t.id) AS bookmark_count,
               NULL::timestamptz AS last_action
        FROM torrents t
        WHERE t.id = $1 AND t.approval_status = 1
        "#,
    )
    .bind(id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    row.ok_or(DomainError::NotFound(id))
}

pub async fn list_files(db: &PgPool, torrent_id: i64) -> DomainResult<Vec<FileRow>> {
    sqlx::query_as::<_, FileRow>(
        "SELECT file_index, path, size FROM files WHERE torrent_id = $1 ORDER BY file_index LIMIT 500",
    )
    .bind(torrent_id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

pub async fn list_thanks(db: &PgPool, torrent_id: i64) -> DomainResult<Vec<ThankRow>> {
    sqlx::query_as::<_, ThankRow>(
        r#"
        SELECT u.username, th.created_at
        FROM thanks th LEFT JOIN users u ON u.id = th.user_id
        WHERE th.torrent_id = $1
        ORDER BY th.created_at DESC LIMIT 50
        "#,
    )
    .bind(torrent_id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CommentRow {
    pub id: i64,
    pub torrent_id: i64,
    pub username: Option<String>,
    pub body: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_comments(
    db: &PgPool,
    torrent_id: i64,
    limit: i64,
) -> DomainResult<Vec<CommentRow>> {
    sqlx::query_as::<_, CommentRow>(
        "SELECT c.id, c.torrent_id, u.username, c.body, c.created_at \
         FROM comments c LEFT JOIN users u ON u.id = c.user_id \
         WHERE c.torrent_id = $1 ORDER BY c.id DESC LIMIT $2",
    )
    .bind(torrent_id)
    .bind(limit.clamp(1, MAX_LIMIT))
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

pub async fn add_comment(
    db: &PgPool,
    torrent_id: i64,
    user_id: i64,
    body: &str,
) -> DomainResult<i64> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1 AND approval_status = 1)",
    )
    .bind(torrent_id)
    .fetch_one(db)
    .await
    .unwrap_or(false);
    if !exists {
        return Err(DomainError::NotFound(torrent_id));
    }
    if body.trim().is_empty() {
        return Err(DomainError::Validation("评论不能为空".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO comments (torrent_id, user_id, body) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(torrent_id)
    .bind(user_id)
    .bind(body)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(id)
}

pub async fn thank(db: &PgPool, torrent_id: i64, user_id: i64) -> DomainResult<()> {
    let res = sqlx::query(
        "INSERT INTO thanks (torrent_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(torrent_id)
    .bind(user_id)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if res.rows_affected() == 0 {
        return Err(DomainError::AlreadyThanked);
    }
    Ok(())
}

pub async fn bookmark(db: &PgPool, torrent_id: i64, user_id: i64, on: bool) -> DomainResult<()> {
    if on {
        sqlx::query(
            "INSERT INTO bookmarks (user_id, torrent_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(torrent_id)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    } else {
        sqlx::query("DELETE FROM bookmarks WHERE user_id = $1 AND torrent_id = $2")
            .bind(user_id)
            .bind(torrent_id)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    Ok(())
}

/// 站点统计（M10 概览，旧站首页口径）
pub async fn site_stats(db: &PgPool) -> DomainResult<serde_json::Value> {
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE status < 2")
        .fetch_one(db)
        .await
        .unwrap_or(0);
    let torrents: i64 =
        sqlx::query_scalar("SELECT count(*) FROM torrents WHERE approval_status = 1")
            .fetch_one(db)
            .await
            .unwrap_or(0);
    let dead: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents WHERE approval_status = 1 AND seeders = 0",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let seed_size: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(t.size), 0) FROM torrents t \
         JOIN (SELECT DISTINCT torrent_id FROM snatches WHERE seeding) s ON s.torrent_id = t.id",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    Ok(serde_json::json!({
        "users": users, "torrents": torrents, "dead": dead, "seed_size": seed_size
    }))
}
