//! 火花账本管线（M11）：spend/earn 及 _tx 事务版（资金动账唯一入口）。
//! 从 economy_http.rs 按域拆出。

use super::spend::SpendOutcome;
use crate::errors::{DomainError, DomainResult};

/// 经济路由（挂载进主 /api/v1 scope，单 scope 避免遮蔽）

/// spend_spark 的传入事务版本：调用方把扣款与其它写操作并入同一事务
/// （如求种悬赏冻结与建单原子化）。语义/锁序与 spend_spark 完全一致。
pub async fn spend_spark_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    amount: i64,
    kind: &str,
    idem: &str,
    ref_type: &str,
    ref_id: i64,
) -> DomainResult<SpendOutcome> {
    let balance: i64 = sqlx::query_scalar(
        "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等检查查询失败必须报错（P2 审计）：unwrap_or(false) 会把「查询失败」当成
    // 「未消费」——跨请求重放可击穿幂等。fail-close：宁可 5xx 也不冒双扣风险。
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
    )
    .bind(idem)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if exists {
        return Ok(SpendOutcome::Replayed);
    }
    if balance < amount {
        return Err(DomainError::InsufficientSpark);
    }
    sqlx::query(
        "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \
         VALUES (nextval('spark_ledger_id_seq'), $1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(user_id)
    .bind(-amount)
    .bind(kind)
    .bind(ref_type)
    .bind(ref_id)
    .bind(idem)
    .bind(balance - amount)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE users SET spark_balance = spark_balance - $2 WHERE id = $1",
    )
    .bind(user_id)
    .bind(amount)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(SpendOutcome::Spent)
}
