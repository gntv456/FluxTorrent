//! 火花扣款入口（M11，非事务版）。
//! 从 economy_http.rs 按域拆出。

use crate::errors::{DomainError, DomainResult};
use sqlx::PgPool;

pub async fn spend_spark(
    db: &PgPool,
    user_id: i64,
    amount: i64,
    kind: &str,
    idem: &str,
    ref_type: &str,
    ref_id: i64,
) -> DomainResult<SpendOutcome> {
    let mut tx = db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等检查必须在余额检查之前：已成功扣过的键在余额不足时也应返回重放，
    // 而不是误报「余额不足」（P0：防并发双扣的锁序不变，行锁仍先取）
    let balance: i64 = sqlx::query_scalar(
        "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等检查查询失败必须报错（P2 审计，同 tx 版口径）
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
    )
    .bind(idem)
    .fetch_one(&mut *tx)
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
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE users SET spark_balance = spark_balance - $2 WHERE id = $1",
    )
    .bind(user_id)
    .bind(amount)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(SpendOutcome::Spent)
}

/// earn_spark 的传入事务版本：调用方把入账与其它写操作并入同一事务（签到/支取/收获）。
/// 语义/锁序与 earn_spark 完全一致；返回 SpendOutcome 复用「Spent=首次入账 /
/// Replayed=幂等键已存在」口径，调用方据 Replayed 处理撕裂残留。
pub async fn earn_spark_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    amount: i64,
    kind: &str,
    idem: &str,
) -> DomainResult<SpendOutcome> {
    let balance: i64 = sqlx::query_scalar(
        "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等检查必须在行锁之后（P0：防并发双入账）
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
    sqlx::query(
        "INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key, balance_after) \
         VALUES (nextval('spark_ledger_id_seq'), $1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(amount)
    .bind(kind)
    .bind(idem)
    .bind(balance + amount)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1",
    )
    .bind(user_id)
    .bind(amount)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(SpendOutcome::Spent)
}

/// 入账（签到/利息/奖励）
pub async fn earn_spark(
    db: &PgPool,
    user_id: i64,
    amount: i64,
    kind: &str,
    idem: &str,
) -> DomainResult<()> {
    let mut tx = db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let balance: i64 = sqlx::query_scalar(
        "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等检查必须在行锁之后（P0：防并发双入账）
    // 幂等检查查询失败必须报错（P2 审计，同 tx 版口径）
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
    )
    .bind(idem)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if exists {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key, balance_after) \
         VALUES (nextval('spark_ledger_id_seq'), $1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(amount)
    .bind(kind)
    .bind(idem)
    .bind(balance + amount)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1",
    )
    .bind(user_id)
    .bind(amount)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 动账核心：余额充足校验 + 负流水 + 余额快照更新（单事务）。
/// 幂等键唯一约束（shop_orders/应用层先查）防重复扣款。
/// 扣款结果：区分真实扣款与幂等重放（调用方据此决定是否执行副作用）。
/// must_use：丢弃该返回值继续执行副作用 = 幂等重放印钞口（审计 P0 整类缺陷的根因）。
#[must_use = "重放（Replayed）时不会再扣款，丢弃返回值继续执行副作用会造成资金凭空入账"]
pub enum SpendOutcome {
    Spent,
    Replayed,
}

/// 发放侧对 `Replayed` 的处置：**当成失败**，让所在事务一起回滚。
///
/// 发放/入账的幂等键都带 uuid 或批次锚点，正常路径不可能重放；真撞上只可能是
/// 请求里有重复目标或键被复用。此时若继续报 200，就是「接口成功而用户零到账」
/// ——发放面最贵的一类缺陷（与 0291 实测 A1 同型）。
pub fn expect_spent(out: SpendOutcome) -> DomainResult<()> {
    match out {
        SpendOutcome::Spent => Ok(()),
        SpendOutcome::Replayed => Err(DomainError::Validation(
            "发放未生效：幂等键重复（请检查目标名单里是否有重复用户）".into(),
        )),
    }
}
