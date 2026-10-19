//! 术语治理：版本快照与回灌（自 terms_gov.rs 按域拆出，守 300 行门禁）。
//!
//! 【回灌为什么必须自己记那一版】P-402：HTTP 层原先是「先 `push_version`
//! 再 `restore_terms`」两步各自 autocommit。restore 一旦失败（快照损坏、
//! 口径漂移、插入报错），版本史里就留下一行 `rollback / restore -> vN`，
//! 而表体一行都没动——实测灌一条坏快照（`{}`）后 restore 回 400/1002
//! 「版本快照格式不正确」，`term_versions` 却从 1 涨到 2。版本列表是站长
//! 判断「谁能回滚到这一版」的唯一依据，多一条没发生过的记录，就会有人照着
//! 它回滚到一段从未存在过的形态。现在「记当前态」与「回灌」在同一事务里：
//! 要么一起成立，要么一起不留痕。

use super::Repo;
use crate::errors::{DomainError, DomainResult};

fn int(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

/// 当前版本号（乐观锁用）。
pub async fn current_version(repo: &Repo) -> DomainResult<i64> {
    let v: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) FROM term_versions",
    )
    .fetch_one(&repo.db)
    .await
    .map_err(int)?;
    Ok(v)
}

/// 版本列表（新到旧）。
pub async fn list_versions(
    repo: &Repo,
    limit: i64,
) -> DomainResult<Vec<VersionRow>> {
    Ok(sqlx::query_as(
        "SELECT version, source, note, changed, created_at, created_by \
         FROM term_versions ORDER BY version DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(&repo.db)
    .await
    .map_err(int)?)
}

/// 一个版本的元信息。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct VersionRow {
    pub version: i64,
    pub source: String,
    pub note: String,
    pub changed: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub created_by: Option<i64>,
}

/// 取某一版的全量快照。
pub async fn version_snapshot(
    repo: &Repo,
    version: i64,
) -> DomainResult<Option<serde_json::Value>> {
    Ok(sqlx::query_scalar(
        "SELECT snapshot FROM term_versions WHERE version = $1",
    )
    .bind(version)
    .fetch_optional(&repo.db)
    .await
    .map_err(int)?
    .flatten())
}

/// 写一份版本快照（各改动口在自己的事务外调用时用这一枚）。
pub async fn push_version(
    repo: &Repo,
    source: &str,
    note: &str,
    changed: i32,
    created_by: Option<i64>,
) -> DomainResult<i64> {
    let mut tx = repo.db.begin().await.map_err(int)?;
    let v =
        push_version_tx(&mut tx, source, note, changed, created_by).await?;
    tx.commit().await.map_err(int)?;
    Ok(v)
}

/// 事务内版本写入：快照取「调用那一刻的 site_terms 全量」，
/// 所以必须在任何覆盖动作**之前**、且在同一个事务里调用。
async fn push_version_tx(
    tx: &mut sqlx::PgTransaction<'_>,
    source: &str,
    note: &str,
    changed: i32,
    created_by: Option<i64>,
) -> DomainResult<i64> {
    let snapshot: serde_json::Value = serde_json::json!({
        "terms": sqlx::query_as::<_, (String, String, bool)>(
            "SELECT canonical, replacement, enabled FROM site_terms \
             ORDER BY canonical",
        )
        .fetch_all(&mut **tx)
        .await
        .map_err(int)?
        .into_iter()
        .map(|(c, r, e)| serde_json::json!({
            "canonical": c, "replacement": r, "enabled": e,
        }))
        .collect::<Vec<_>>(),
    });
    Ok(sqlx::query_scalar::<_, i64>(
        "INSERT INTO term_versions (source, note, snapshot, changed, \
                                     created_by) \
         VALUES ($1, $2, $3::jsonb, $4, $5) RETURNING version",
    )
    .bind(source)
    .bind(note)
    .bind(snapshot.to_string())
    .bind(changed)
    .bind(created_by)
    .fetch_one(&mut **tx)
    .await
    .map_err(int)?)
}

/// 回灌某一版快照，并在**同一事务**里先把「回灌前的当前态」记成一版。
///
/// 返回 `(新记录的版本号, 回灌行数)`。快照格式不对、或中途任何一步失败
/// ⇒ 整笔回滚：既不动表，也不在版本史里留这条没发生过的记录。
///
/// 全表替换（而非补差）是快照语义本身的要求：只补差会漏掉
/// 「快照里有、现在没有」的规则。
pub async fn restore_marked(
    repo: &Repo,
    snapshot: &serde_json::Value,
    note: &str,
    created_by: Option<i64>,
) -> DomainResult<(i64, u64)> {
    // 先验格式：坏快照根本不该进事务（否则白占一版）。
    let rows = snapshot
        .get("terms")
        .and_then(|v| v.as_array())
        .ok_or_else(|| DomainError::Validation("版本快照格式不正确".into()))?;
    let mut tx = repo.db.begin().await.map_err(int)?;
    let v = push_version_tx(&mut tx, "rollback", note, 0, created_by).await?;
    sqlx::query("DELETE FROM site_terms")
        .execute(&mut *tx)
        .await
        .map_err(int)?;
    let mut n = 0u64;
    for row in rows {
        let c = row.get("canonical").and_then(|v| v.as_str()).unwrap_or("");
        let r = row
            .get("replacement")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let en = row.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
        if c.is_empty() || r.is_empty() {
            continue;
        }
        sqlx::query(
            "INSERT INTO site_terms \
             (canonical, replacement, enabled, descr, sort) \
             VALUES ($1, $2, $3, '', 100)",
        )
        .bind(c)
        .bind(r)
        .bind(en)
        .execute(&mut *tx)
        .await
        .map_err(int)?;
        n += 1;
    }
    tx.commit().await.map_err(int)?;
    Ok((v, n))
}
