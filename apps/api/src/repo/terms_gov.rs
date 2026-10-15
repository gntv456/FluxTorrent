//! 术语治理的 SQL 唯一居所（FRAME §2 H 层不下沉 sqlx）。
//!
//! 消费方：`staff_http/terms_gov.rs`（覆盖率巡检 / 版本列表 / 键登记）。
//! 判定在 `domain::terms`（状态迁移 / 覆盖率），本文件只读写。
//!
//! 【版本快照的写入纪律】每次改动都留一份全量快照。diff 靠
//! `version_versions` 的相邻两行对比，恢复则直接回灌某一版——
//! 有了快照，「改错一个词只能手工改回来」才成立。

use super::Repo;
use crate::errors::{DomainError, DomainResult};

/// 键的一行。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct KeyRow {
    pub key: String,
    pub display_zh: String,
    pub category: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub retired_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 列全部键（按分类 + 状态排序）。
pub async fn list_keys(repo: &Repo) -> DomainResult<Vec<KeyRow>> {
    Ok(sqlx::query_as(
        "SELECT key, display_zh, category, status, created_at, \
                updated_at, retired_at \
         FROM term_keys ORDER BY category, status, key",
    )
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 登记 / 更新一个键（幂等 upsert）。
pub async fn upsert_key(
    repo: &Repo,
    key: &str,
    display_zh: &str,
    category: &str,
) -> DomainResult<()> {
    sqlx::query(
        "INSERT INTO term_keys (key, display_zh, category) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (key) DO UPDATE \
         SET display_zh = EXCLUDED.display_zh, \
             category = EXCLUDED.category, \
             updated_at = now()",
    )
    .bind(key)
    .bind(display_zh)
    .bind(category)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 迁移键状态（状态机判定在 domain::terms，本函数只执行）。
///
/// `retired_at` 与 status 必须同时落——0386 迁移里有 CHECK 约束兜着，
/// 这里显式写是为了让「为什么 retired_at 有值」一眼可读。
pub async fn set_status(
    repo: &Repo,
    key: &str,
    status: &str,
) -> DomainResult<u64> {
    let n = sqlx::query(
        "UPDATE term_keys \
         SET status = $2, \
             retired_at = CASE WHEN $2 = 'retired' THEN now() ELSE NULL END, \
             updated_at = now() \
         WHERE key = $1",
    )
    .bind(key)
    .bind(status)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    Ok(n)
}

/// 当前版本号（乐观锁用）。
pub async fn current_version(repo: &Repo) -> DomainResult<i64> {
    let v: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version), 0) FROM term_versions",
    )
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
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
    .map_err(|e| DomainError::Internal(e.into()))?)
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
    .map_err(|e| DomainError::Internal(e.into()))?
    .flatten())
}

/// 写一份版本快照（每次改动前调用）。
pub async fn push_version(
    repo: &Repo,
    source: &str,
    note: &str,
    changed: i32,
    created_by: Option<i64>,
) -> DomainResult<i64> {
    // 快照取「当前 site_terms 全量」——回灌时无需再拼装。
    let snapshot: serde_json::Value = serde_json::json!({
        "terms": sqlx::query_as::<_, (String, String, bool)>(
            "SELECT canonical, replacement, enabled FROM site_terms \
             ORDER BY canonical",
        )
        .fetch_all(&repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .into_iter()
        .map(|(c, r, e)| serde_json::json!({
            "canonical": c, "replacement": r, "enabled": e,
        }))
        .collect::<Vec<_>>(),
    });
    let v: i64 = sqlx::query_scalar(
        "INSERT INTO term_versions (source, note, snapshot, changed, \
                                     created_by) \
         VALUES ($1, $2, $3::jsonb, $4, $5) RETURNING version",
    )
    .bind(source)
    .bind(note)
    .bind(snapshot.to_string())
    .bind(changed)
    .bind(created_by)
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(v)
}

/// 巡检输入的四个计数。
#[derive(Debug, Clone, Copy, sqlx::FromRow)]
pub struct Counts {
    pub total: i64,
    pub active: i64,
    pub deprecated: i64,
    pub retired: i64,
    pub with_rule: i64,
}

/// 取巡检计数。
pub async fn audit_counts(repo: &Repo) -> DomainResult<Counts> {
    Ok(sqlx::query_as(
        "SELECT \
           (SELECT count(*)::bigint FROM term_keys) AS total, \
           (SELECT count(*)::bigint FROM term_keys \
            WHERE status = 'active') AS active, \
           (SELECT count(*)::bigint FROM term_keys \
            WHERE status = 'deprecated') AS deprecated, \
           (SELECT count(*)::bigint FROM term_keys \
            WHERE status = 'retired') AS retired, \
           (SELECT count(*)::bigint FROM site_terms WHERE enabled) \
             AS with_rule",
    )
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 引用了已退役键的地方。
///
/// 口径：site_terms 里 enabled 的规则，其 canonical 若在 term_keys
/// 里是 retired，就算一处引用。这是**保守**的近似——真正的引用还包括
/// 前端硬编码，但前端那部分扫描成本高且易漏，保守近似至少能指出
/// 「规则表里还有退役键在生效」这种最该先清的情况。
pub async fn retired_refs(repo: &Repo) -> DomainResult<Vec<String>> {
    Ok(sqlx::query_scalar(
        "SELECT t.canonical FROM site_terms t \
         JOIN term_keys k ON k.key = t.canonical \
         WHERE t.enabled AND k.status = 'retired' \
         ORDER BY t.canonical",
    )
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 落一份巡检报告。
pub async fn push_audit(
    repo: &Repo,
    total: i64,
    active: i64,
    deprecated: i64,
    retired: i64,
    with_rule: i64,
    coverage_pct: f64,
    retired_refs: &[String],
    note: &str,
) -> DomainResult<i64> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO term_audit_reports \
         (total_keys, active_keys, deprecated_keys, retired_keys, \
          with_rule, coverage_pct, retired_refs, note) \
         VALUES ($1, $2, $3, $4, $5, $6, $7::jsonb, $8) RETURNING id",
    )
    .bind(total)
    .bind(active)
    .bind(deprecated)
    .bind(retired)
    .bind(with_rule)
    .bind(coverage_pct)
    .bind(serde_json::json!(retired_refs).to_string())
    .bind(note)
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(id)
}

/// 写入一条规则（upsert）。返回是否真的改变了内容（供版本快照的 changed 计）。
///
/// 与 HTTP 层的旧口径一致：`site_terms` 单表、无版本。0386 补的
/// `term_versions` 要能真正回滚，前提是**每次改动都留快照**——所以
/// 改动口的唯一入口在本文件，handler 只做校验与编排。
pub async fn upsert_term(
    repo: &Repo,
    canonical: &str,
    replacement: &str,
    enabled: bool,
    descr: &str,
    sort: i32,
) -> DomainResult<()> {
    sqlx::query(
        "INSERT INTO site_terms \
         (canonical, replacement, enabled, descr, sort) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (canonical) DO UPDATE SET \
         replacement = EXCLUDED.replacement, \
         enabled = EXCLUDED.enabled, \
         descr = EXCLUDED.descr, sort = EXCLUDED.sort, \
         updated_at = now()",
    )
    .bind(canonical)
    .bind(replacement)
    .bind(enabled)
    .bind(descr)
    .bind(sort)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 切换一条规则的启用位（保留词条）。
pub async fn toggle_term(
    repo: &Repo,
    canonical: &str,
) -> DomainResult<Option<bool>> {
    Ok(sqlx::query_scalar(
        "UPDATE site_terms SET enabled = NOT enabled, updated_at = now() \
         WHERE canonical = $1 RETURNING enabled",
    )
    .bind(canonical)
    .fetch_optional(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 删一条规则，返回删除行数。
pub async fn delete_term(
    repo: &Repo,
    canonical: &str,
) -> DomainResult<u64> {
    Ok(sqlx::query("DELETE FROM site_terms WHERE canonical = $1")
        .bind(canonical)
        .execute(&repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected())
}

/// 当前生效规则条数（版本快照的 changed 计数用）。
pub async fn active_term_count(repo: &Repo) -> DomainResult<i64> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM site_terms WHERE enabled",
    )
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 某条规则是否已登记（给上限判定用）。
pub async fn term_exists(
    repo: &Repo,
    canonical: &str,
) -> DomainResult<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM site_terms WHERE canonical = $1",
    )
    .bind(canonical)
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
        > 0)
}

/// 回灌某一版快照（把 site_terms 整表恢复到该版的形态）。
///
/// 先记一份当前态再覆盖——回滚本身也是一次改动，不留快照的话
/// 「回滚 → 再回滚」就变成了单向操作，这正是版本功能要消灭的东西。
pub async fn restore_terms(
    repo: &Repo,
    snapshot: &serde_json::Value,
) -> DomainResult<u64> {
    let rows = snapshot
        .get("terms")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            DomainError::Validation("版本快照格式不正确".into())
        })?;
    let mut tx = repo.db.begin().await.map_err(|e| {
        DomainError::Internal(e.into())
    })?;
    // 全表替换：快照语义就是「那一刻的完整形态」，只补差会漏掉
    // 「快照里有、现在没有」的规则。
    sqlx::query("DELETE FROM site_terms")
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let mut n = 0u64;
    for row in rows {
        let c = row
            .get("canonical")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let r = row
            .get("replacement")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let en = row
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
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
        .map_err(|e| DomainError::Internal(e.into()))?;
        n += 1;
    }
    tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
    Ok(n)
}

/// 列全部规则（后台首屏）。
///
/// 排序沿用热加载侧的口径：长词优先（先替短的再替长的，避免「新词里
/// 含有旧词」时被截断），再按 sort / canonical。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct TermRow {
    pub canonical: String,
    pub replacement: String,
    pub enabled: bool,
    pub descr: String,
    pub sort: i32,
}

pub async fn list_terms(repo: &Repo) -> DomainResult<Vec<TermRow>> {
    Ok(sqlx::query_as(
        "SELECT canonical, replacement, enabled, descr, sort \
         FROM site_terms \
         ORDER BY length(canonical) DESC, sort, canonical",
    )
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}
