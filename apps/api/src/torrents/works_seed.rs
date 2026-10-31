//! 作品聚合的种子行读口（列表批量 + 详情单作品）。
//!
//! 行 = 列表行 + 简介截图（前 4 张）。SQL 文本由调用方传入——
//! `torrent_http/works_rows_sql.rs` 是 H 层的 SQL 构造器（它不许出现
//! `sqlx`），解码与执行收在种子域这里。截图只属于作品聚合读口，
//! 不塞进共享的 `TorrentRow`（`/torrents` 列表与详情共用那个结构）。

use serde::Serialize;
use sqlx::PgPool;

use super::TorrentRow;

/// 作品种子行：序列化时 flatten 回与列表行同形的 JSON，前端一套渲染通吃。
#[derive(Debug, Serialize)]
pub struct WorkSeed {
    #[serde(flatten)]
    tor: TorrentRow,
    #[serde(skip_serializing_if = "Option::is_none")]
    screenshots: Option<serde_json::Value>,
}

impl WorkSeed {
    fn new(tor: TorrentRow, shots: Option<serde_json::Value>) -> Self {
        Self {
            tor,
            screenshots: shots,
        }
    }
}

/// 详情腿的查询行：flatten 列表行 + 截图列。
#[derive(Debug, sqlx::FromRow)]
struct SeedRow {
    #[sqlx(flatten)]
    tor: TorrentRow,
    screenshots: Option<serde_json::Value>,
}

/// 列表腿的批量行：多一列 work_key。
#[derive(Debug, sqlx::FromRow)]
struct BatchSeedRow {
    work_key: String,
    #[sqlx(flatten)]
    tor: TorrentRow,
    screenshots: Option<serde_json::Value>,
}

/// 作品列表腿：按作品键批量取行，返回 `(work_key, seed)` 由调用方归堆。
pub async fn batch(
    db: &PgPool,
    sql: &str,
    work_keys: &[String],
) -> Result<Vec<(String, WorkSeed)>, sqlx::Error> {
    let rows: Vec<BatchSeedRow> =
        sqlx::query_as(sql).bind(work_keys).fetch_all(db).await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.work_key, WorkSeed::new(r.tor, r.screenshots)))
        .collect())
}

/// 作品详情腿：单个作品的全部种子行。
pub async fn for_work(
    db: &PgPool,
    sql: &str,
    key: &str,
) -> Result<Vec<WorkSeed>, sqlx::Error> {
    let rows: Vec<SeedRow> =
        sqlx::query_as(sql).bind(key).fetch_all(db).await?;
    Ok(rows
        .into_iter()
        .map(|r| WorkSeed::new(r.tor, r.screenshots))
        .collect())
}
