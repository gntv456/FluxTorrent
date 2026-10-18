//! webhook 出网白名单 / 投递台账 / 健康度的 SQL 唯一居所。
//!
//! 消费方：`ops_webhook.rs`（发送）、`admin_http/webhook_ops.rs`（管理面）。
//! 判据在 `domain::webhook`（出网是否允许、签名怎么算）。

use super::Repo;
use crate::errors::{DomainError, DomainResult};

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

/// 读一个 webhook 相关设置（空值视作未配置）。
pub async fn setting(repo: &Repo, name: &str) -> DomainResult<String> {
    Ok(sqlx::query_scalar(
        "SELECT NULLIF(value, '') FROM site_settings WHERE name = $1",
    )
    .bind(name)
    .fetch_optional(&repo.db)
    .await
    .map_err(internal)?
    .unwrap_or_default())
}

/// 出网白名单网段（无记录 = 只允许公网）。
pub async fn egress_cidrs(repo: &Repo) -> DomainResult<Vec<String>> {
    Ok(sqlx::query_scalar(
        "SELECT cidr::text FROM webhook_egress_rules ORDER BY id",
    )
    .fetch_all(&repo.db)
    .await
    .map_err(internal)?)
}

/// 登记一条白名单网段（幂等）。
pub async fn add_egress_cidr(
    repo: &Repo,
    cidr: &str,
    note: &str,
) -> DomainResult<bool> {
    // 语法校验交给 PG 的 CIDR 类型：写不进去就是写不进去，
    // 不在应用层维护第二套 CIDR 解析（两套口径迟早不一致）。
    let n = sqlx::query(
        "INSERT INTO webhook_egress_rules (cidr, note) \
         VALUES ($1::cidr, $2) ON CONFLICT (cidr) DO NOTHING",
    )
    .bind(cidr)
    .bind(note)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Validation(format!("网段格式不合法：{e}")))?
    .rows_affected();
    Ok(n > 0)
}

/// 删一条白名单网段。
pub async fn del_egress_cidr(repo: &Repo, cidr: &str) -> DomainResult<u64> {
    Ok(
        sqlx::query("DELETE FROM webhook_egress_rules WHERE cidr = $1::cidr")
            .bind(cidr)
            .execute(&repo.db)
            .await
            .map_err(internal)?
            .rows_affected(),
    )
}

/// 记一次投递结果。
#[allow(clippy::too_many_arguments)]
pub async fn record_delivery(
    repo: &Repo,
    channel: &str,
    event: &str,
    status: &str,
    attempts: i32,
    elapsed_ms: i32,
    detail: &str,
) -> DomainResult<()> {
    sqlx::query(
        "INSERT INTO webhook_deliveries \
         (channel, event, status, attempts, elapsed_ms, detail) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(channel)
    .bind(event)
    .bind(status)
    .bind(attempts)
    .bind(elapsed_ms)
    .bind(detail)
    .execute(&repo.db)
    .await
    .map_err(internal)?;
    Ok(())
}

/// 该出口的连续失败次数（只看最近一条成功之后的失败）。
pub async fn consecutive_failures(
    repo: &Repo,
    channel: &str,
) -> DomainResult<i64> {
    let n: Option<i64> = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM webhook_deliveries d \
         WHERE d.channel = $1 \
           AND d.created_at > COALESCE((SELECT max(created_at) FROM webhook_deliveries \
             WHERE channel = $1 AND status = 'ok'), 'epoch'::timestamptz) \
           AND d.status <> 'ok'",
    )
    .bind(channel)
    .fetch_optional(&repo.db)
    .await
    .map_err(internal)?;
    Ok(n.unwrap_or(0))
}

/// 某出口是否已被自动停用（连续失败达阈值；阈值 0 = 不自动停用）。
pub async fn is_disabled(
    repo: &Repo,
    channel: &str,
    threshold: i64,
) -> DomainResult<bool> {
    if threshold <= 0 {
        return Ok(false);
    }
    Ok(consecutive_failures(repo, channel).await? >= threshold)
}

/// 最近投递台账（管理面读）。
pub async fn recent_deliveries(
    repo: &Repo,
    limit: i64,
) -> DomainResult<Vec<serde_json::Value>> {
    let rows: Vec<(String, String, String, i32, i32, String, String)> =
        sqlx::query_as(
            "SELECT channel, event, status, attempts, elapsed_ms, detail, \
                    created_at::text \
             FROM webhook_deliveries ORDER BY created_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&repo.db)
        .await
        .map_err(internal)?;
    Ok(rows
        .into_iter()
        .map(|(c, e, s, a, ms, d, at)| {
            serde_json::json!({
                "channel": c, "event": e, "status": s,
                "attempts": a, "elapsed_ms": ms,
                "detail": d, "created_at": at,
            })
        })
        .collect())
}

/// 清掉某出口的失败计数（改完配置后手工恢复用）。
pub async fn reset_failures(repo: &Repo, channel: &str) -> DomainResult<u64> {
    Ok(sqlx::query(
        "DELETE FROM webhook_deliveries \
         WHERE channel = $1 AND status <> 'ok' \
           AND created_at > COALESCE((SELECT max(created_at) \
             FROM webhook_deliveries WHERE channel = $1 AND status = 'ok'), \
             'epoch'::timestamptz)",
    )
    .bind(channel)
    .execute(&repo.db)
    .await
    .map_err(internal)?
    .rows_affected())
}
