//! 反作弊规则集版本 / 影子 / 回测的 SQL 唯一居所。
//!
//! 消费方：`admin_http/cheat_ruleset_admin.rs`。
//! 判据在 worker `risk_score.rs`（打分与回测计算）。

use super::Repo;
use crate::errors::{DomainError, DomainResult};

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

/// 影子模式开关。
pub async fn shadow_mode(repo: &Repo) -> DomainResult<bool> {
    let v: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'risk_shadow_mode'",
    )
    .fetch_optional(&repo.db)
    .await
    .map_err(internal)?;
    Ok(matches!(v.as_deref(), Some(s) if s != "0" && s != "no"))
}

pub async fn set_shadow_mode(repo: &Repo, on: bool) -> DomainResult<()> {
    sqlx::query(
        "INSERT INTO site_settings (name, value) \
         VALUES ('risk_shadow_mode', $1) \
         ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(if on { "1" } else { "0" })
    .execute(&repo.db)
    .await
    .map_err(internal)?;
    Ok(())
}

/// 版本列表（新到旧）。params 原样回显：这是站长要对比的东西。
pub async fn list_rulesets(
    repo: &Repo,
) -> DomainResult<Vec<serde_json::Value>> {
    let rows: Vec<(i32, serde_json::Value, String, bool, Option<String>)> =
        sqlx::query_as(
            "SELECT version, params, note, active, \
                    activated_at::text FROM cheat_rulesets \
             ORDER BY version DESC",
        )
        .fetch_all(&repo.db)
        .await
        .map_err(internal)?;
    Ok(rows
        .into_iter()
        .map(|(version, params, note, active, at)| {
            serde_json::json!({
                "version": version, "params": params, "note": note,
                "active": active, "activated_at": at,
            })
        })
        .collect())
}

/// 存一版草稿：以**当前 site_settings 的 risk_*** 为基线 + 覆盖。
///
/// 基线取 site_settings 而非上一版 params：站长改过 site_settings
/// 还没存版时，草稿应当包含那些改动——否则「改了设置但建不出
/// 对应版本」，版本与实际生效值会悄悄分叉。
pub async fn create_ruleset(
    repo: &Repo,
    overrides: &std::collections::BTreeMap<String, String>,
    note: &str,
) -> DomainResult<i32> {
    let mut params: serde_json::Map<String, serde_json::Value> =
        serde_json::Map::new();
    // 用 r"..." 原始串：普通 Rust 串里的 \_ 会被当成转义序列，
    // 而 PG 需要的就是反斜杠本身（LIKE 'risk\_%' 里 \_ 转义下划线）。
    let rows: Vec<(String, String)> = sqlx::query_as(
        r"SELECT name, value FROM site_settings
           WHERE name LIKE 'risk\_%'
             AND name NOT IN ('risk_shadow_mode','risk_shadow_max_ver')",
    )
    .fetch_all(&repo.db)
    .await
    .map_err(internal)?;
    for (k, v) in rows {
        params.insert(k, serde_json::Value::String(v));
    }
    for (k, v) in overrides {
        params.insert(k.clone(), serde_json::Value::String(v.clone()));
    }
    let next: i32 = sqlx::query_scalar(
        "SELECT COALESCE(max(version), 0) + 1 FROM cheat_rulesets",
    )
    .fetch_one(&repo.db)
    .await
    .map_err(internal)?;
    sqlx::query(
        "INSERT INTO cheat_rulesets (version, params, note, active) \
         VALUES ($1, $2::jsonb, $3, false)",
    )
    .bind(next)
    .bind(serde_json::Value::Object(params).to_string())
    .bind(note)
    .execute(&repo.db)
    .await
    .map_err(internal)?;
    Ok(next)
}

/// 切换生效版本：先关旧的再开新的。
///
/// 两条同时 active 会让分数行的 ruleset_ver 指向不明，故用事务
/// 保证「关旧 + 开新」原子。
pub async fn activate(repo: &Repo, version: i32) -> DomainResult<u64> {
    let mut tx = repo.db.begin().await.map_err(internal)?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM cheat_rulesets WHERE version = $1)",
    )
    .bind(version)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    if !exists {
        return Err(DomainError::NotFound(version as i64));
    }
    sqlx::query("UPDATE cheat_rulesets SET active = false \
                 WHERE active")
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    let n = sqlx::query(
        "UPDATE cheat_rulesets SET active = true, activated_at = now() \
         WHERE version = $1",
    )
    .bind(version)
    .execute(&mut *tx)
    .await
    .map_err(internal)?
    .rows_affected();
    tx.commit().await.map_err(internal)?;
    Ok(n)
}

/// 影子对比计数。
pub async fn shadow_summary(repo: &Repo) -> DomainResult<ShadowSummary> {
    Ok(sqlx::query_as(
        "SELECT count(*)::int AS total, \
                count(*) FILTER (WHERE would_open)::int AS would_open \
         FROM cheat_shadow_scores",
    )
    .fetch_one(&repo.db)
    .await
    .map_err(internal)?)
}

#[derive(sqlx::FromRow)]
pub struct ShadowSummary {
    pub total: i32,
    pub would_open: i32,
}

/// 影子分数最高的一批（运营最关心「谁会因为改权重被开案」）。
pub async fn shadow_top(
    repo: &Repo,
    limit: i64,
) -> DomainResult<Vec<serde_json::Value>> {
    let rows: Vec<(i64, i32, String, bool, Option<String>, i32)> =
        sqlx::query_as(
            "SELECT s.user_id, s.score, s.band, s.would_open, \
                    u.username, s.delta_vs_live \
             FROM cheat_shadow_scores s \
             LEFT JOIN users u ON u.id = s.user_id \
             ORDER BY s.would_open DESC, s.score DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&repo.db)
        .await
        .map_err(internal)?;
    Ok(rows
        .into_iter()
        .map(|(uid, score, band, open, name, delta)| {
            serde_json::json!({
                "user_id": uid, "username": name, "score": score,
                "band": band, "would_open": open, "delta_vs_live": delta,
            })
        })
        .collect())
}

/// 登记一次回测请求（worker 稍后算并回填）。
///
/// 用 `total_users = -1` 表示「已排队未完成」——回测要扫窗口内全部证据，
/// 比评分重得多，不能在请求里同步算。不能用 diffs=NULL 做「未完成」标记：
/// 列是 NOT NULL（0391 建表即如此），写 NULL 直接 500。
pub async fn enqueue_backtest(
    repo: &Repo,
    from_ver: i32,
    to_ver: i32,
    window_days: i32,
) -> DomainResult<i64> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO cheat_backtest_runs \
         (from_ver, to_ver, window_days, total_users) \
         VALUES ($1, $2, $3, -1) RETURNING id",
    )
    .bind(from_ver)
    .bind(to_ver)
    .bind(window_days)
    .fetch_one(&repo.db)
    .await
    .map_err(internal)?;
    Ok(id)
}

/// 回测历史（新到旧）。
pub async fn list_backtests(
    repo: &Repo,
    limit: i64,
) -> DomainResult<Vec<serde_json::Value>> {
    let rows: Vec<(
        i64, i32, i32, i32, i32, i32, i32, i32, Option<i64>, String,
    )> =
        sqlx::query_as(
            "SELECT id, from_ver, to_ver, window_days, total_users, \
                    would_open, band_up, band_down, \
                    CASE WHEN total_users < 0 THEN NULL \
                         ELSE jsonb_array_length(diffs)::bigint END, \
                    created_at::text \
             FROM cheat_backtest_runs ORDER BY created_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&repo.db)
        .await
        .map_err(internal)?;
    Ok(rows
        .into_iter()
        .map(
            |(id, from, to, days, total, open, up, down, diff_n, at)| {
                serde_json::json!({
                    "id": id, "from_ver": from, "to_ver": to,
                    "window_days": days, "total_users": total,
                    "would_open": open, "band_up": up, "band_down": down,
                    "diff_count": diff_n, "created_at": at,
                    // -1 = 排队占位（enqueue 写入，等 worker 回填）
                    "done": total >= 0,
                })
            },
        )
        .collect())
}
