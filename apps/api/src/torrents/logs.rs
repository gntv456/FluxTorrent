//! 抓轨日志读路径（0312）。
//!
//! 两块：详情页折叠面板要的**元数据清单**（进 aggregate 共享段，不含正文），
//! 与点开某一份日志时才取的**正文**（单碟 EAC 日志可上百 KiB，全部内联进
//! 首屏会把缓存体积打爆——正文一律懒加载，与 peers 同一范式）。

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 详情页/审核台用的日志一行（无正文）。
/// Deserialize：aggregate 共享段 Redis 缓存反序列化用。
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct LogRow {
    pub ordinal: i32,
    pub filename: String,
    pub engine: String,
    /// 0–100；null = 无法定分（认不出/不支持/日志残缺）
    pub log_score: Option<i16>,
    pub tracks: i32,
    /// [{code,msg}] —— msg 已是给人读的一句话，前端直接铺
    pub issues: serde_json::Value,
    pub size: i64,
}

/// 日志清单（aggregate 内调用：可见性已由 get_torrent 那一道门判掉，
/// 与 files/nfo 同一口径）。
pub async fn list_logs(
    db: &PgPool,
    torrent_id: i64,
) -> DomainResult<Vec<LogRow>> {
    sqlx::query_as::<_, LogRow>(
        "SELECT ordinal, filename, engine, log_score, tracks, issues, size \
         FROM torrent_logs WHERE torrent_id = $1 ORDER BY ordinal LIMIT 24",
    )
    .bind(torrent_id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

/// 某一份日志（元数据 + 正文）。
///
/// 子资源必须走可见性统一口径，否则待审种的日志内容对全站任意登录用户可查
/// ——0288 修过的同一类「主详情 404、子资源 200」劈叉。
pub async fn get_log(
    db: &PgPool,
    torrent_id: i64,
    ordinal: i32,
    viewer: (i64, bool),
) -> DomainResult<Option<(LogRow, String)>> {
    super::assert_visible(db, torrent_id, viewer).await?;
    let row = sqlx::query(
        "SELECT ordinal, filename, engine, log_score, tracks, issues, size, \
         body FROM torrent_logs WHERE torrent_id = $1 AND ordinal = $2",
    )
    .bind(torrent_id)
    .bind(ordinal)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(r) = row else { return Ok(None) };
    use sqlx::Row;
    Ok(Some((
        LogRow {
            ordinal: r.get("ordinal"),
            filename: r.get("filename"),
            engine: r.get("engine"),
            log_score: r.get("log_score"),
            tracks: r.get("tracks"),
            issues: r.get("issues"),
            size: r.get("size"),
        },
        r.get("body"),
    )))
}
