//! 赛事详情（GET /matches/{id}）的 SQL 唯一居所。
//!
//! 2026-10-16 下沉：见 follows.rs 头注。

use super::super::Repo;
use crate::errors::{DomainError, DomainResult};

/// 该场次整行（to_jsonb 整取，前端要的字段随表演进）。
pub async fn match_json(
    repo: &Repo,
    id: i64,
) -> DomainResult<Option<serde_json::Value>> {
    let row: Option<(serde_json::Value,)> = sqlx::query_as(
        "SELECT to_jsonb(m) FROM sport_matches m WHERE m.id = $1",
    )
    .bind(id)
    .fetch_optional(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(row.map(|(v,)| v))
}

/// 该场次的过审种（全场/集锦/回放并列）。
pub async fn match_items(
    repo: &Repo,
    id: i64,
) -> DomainResult<Vec<(i64, String, i64, i32, i32)>> {
    Ok(sqlx::query_as(
        "SELECT id, name, size, seeders, times_completed FROM torrents \
         WHERE match_id = $1 AND approval_status = 1 \
         ORDER BY id DESC LIMIT 100",
    )
    .bind(id)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}
