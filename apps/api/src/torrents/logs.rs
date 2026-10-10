//! 抓轨日志读路径（0312；0337 加人工改判口径）。
//!
//! 两块：详情页折叠面板要的**元数据清单**（进 aggregate 共享段，不含正文），
//! 与点开某一份日志时才取的**正文**（单碟 EAC 日志可上百 KiB，全部内联进
//! 首屏会把缓存体积打爆——正文一律懒加载，与 peers 同一范式）。
//!
//! 0337 起每行同时带出人工改判结果：`log_score` 是引擎原始判分，
//! `adjusted_score` 是版主裁定的口径（空 = 未改判）。**消费方取
//! `COALESCE(adjusted_score, log_score)`**，两者都透出以便审计。

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 详情页/审核台用的日志一行（无正文）。
/// Deserialize：aggregate 共享段 Redis 缓存反序列化用——0337 新增的两列
/// 必须带 `serde(default)`，否则旧缓存（无该键）反序列化会整段失效。
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct LogRow {
    pub ordinal: i32,
    pub filename: String,
    pub engine: String,
    /// 0–100；null = 无法定分（认不出/不支持/日志残缺）
    pub log_score: Option<i16>,
    /// 人工改判分（0337）；null = 未改判
    #[serde(default)]
    pub adjusted_score: Option<i16>,
    /// 改判理由（0337）
    #[serde(default)]
    pub adjust_reason: Option<String>,
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
        "SELECT ordinal, filename, engine, log_score, adjusted_score, \
                adjust_reason, tracks, issues, size \
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
        "SELECT ordinal, filename, engine, log_score, adjusted_score, \
                adjust_reason, tracks, issues, size, \
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
            adjusted_score: r.get("adjusted_score"),
            adjust_reason: r.get("adjust_reason"),
            tracks: r.get("tracks"),
            issues: r.get("issues"),
            size: r.get("size"),
        },
        r.get("body"),
    )))
}

/// 版主人工改判（0337）：写 adjusted_* 四列，原始 log_score 不动。
/// `score=None` 表示撤销改判（回到引擎判分）。
pub async fn adjust_log(
    db: &PgPool,
    torrent_id: i64,
    ordinal: i32,
    score: Option<i16>,
    by: i64,
    reason: &str,
) -> DomainResult<bool> {
    let n = sqlx::query(
        "UPDATE torrent_logs SET adjusted_score = $1, adjusted_by = $2, \
                adjust_reason = $3, adjusted_at = now() \
         WHERE torrent_id = $4 AND ordinal = $5",
    )
    .bind(score)
    .bind(by)
    .bind(reason)
    .bind(torrent_id)
    .bind(ordinal)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    Ok(n == 1)
}

/// 判分维度回流（0314 补全）：把 `torrent_logs` 的权威分（多碟取最低，
/// 有效分 = adjusted ?? engine）与 haslog（有日志且 100）同步进
/// `torrent_sections` 的 log_score/haslog 两维度——筛选面（sec_log_score_min
/// 区间 / haslog=true）从此有自动数据，不再依赖用户手填已算好的数。
/// 维度未在 section_kinds 声明的站型（0314 只给 music 族包种了）自动跳过；
/// 幂等：先清后写，重复调用安全。无日志时清掉两维度行（复活/删日志）。
pub async fn sync_log_dims(db: &PgPool, torrent_id: i64) -> DomainResult<()> {
    let has_dims: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM section_kinds \
          WHERE kind IN ('log_score', 'haslog'))",
    )
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !has_dims {
        return Ok(());
    }
    let worst: Option<i16> = sqlx::query_scalar(
        "SELECT min(COALESCE(adjusted_score, log_score)) FROM torrent_logs \
         WHERE torrent_id = $1 AND COALESCE(adjusted_score, log_score) \
           IS NOT NULL",
    )
    .bind(torrent_id)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // Gazelle 口径：有日志、且每一碟都是有效分 100（判不了分的碟不算 100）
    let has_log: bool = sqlx::query_scalar(
        "SELECT count(*) > 0 AND count(*) = count(*) FILTER \
           (WHERE COALESCE(adjusted_score, log_score) = 100) \
           FROM torrent_logs WHERE torrent_id = $1",
    )
    .bind(torrent_id)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut tx = db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "DELETE FROM torrent_sections WHERE torrent_id = $1 \
         AND kind IN ('log_score', 'haslog')",
    )
    .bind(torrent_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(w) = worst {
        sqlx::query(
            "INSERT INTO torrent_sections (torrent_id, kind, value) \
             VALUES ($1, 'log_score', to_jsonb($2::int))",
        )
        .bind(torrent_id)
        .bind(w as i32)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    if has_log {
        sqlx::query(
            "INSERT INTO torrent_sections (torrent_id, kind, value) \
             VALUES ($1, 'haslog', 'true'::jsonb)",
        )
        .bind(torrent_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}
