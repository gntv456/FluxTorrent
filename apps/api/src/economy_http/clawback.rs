//! 火花「收回/清算」专用原语（P-415）。
//!
//! 为什么不让收回走 `earn_spark`：`checked_balance` 只防 int64 溢出、**不防负**，
//! 而 `users.spark_balance` 是 `bigint NOT NULL` 却没有 CHECK 约束（同表
//! `publish_credits` 反而有 `>= 0`）。于是三处 `earn_spark(db, uid, -1 / -2 /
//! -(n), …)` 外面再套一层 `let _ =`，效果是：版主删一个零余额会员的帖子，
//! 账面直接变成「会员欠站里火花」（现网 `status=0` 的 `ahm47478` 已经是 -100），
//! 而失败被吞掉、连一条日志都没有。
//!
//! 口径照 `repo::cheat_reverse::apply_reclaim_tx`（P-403 族）：**能收多少收多少、
//! 地板 0**；流水与余额写同一个 applied 值，保住「sum(流水) == 余额」这条对账
//! 不变量（写全额流水、夹零余额会造成账面与流水两本账）；夹零与部分收回都出声。

use sqlx::PgPool;

use crate::economy_http::checked_balance;
use crate::errors::{DomainError, DomainResult};

/// 事务版收回：`requested` 是**正数**（应收回的数额），返回实际收回额。
pub async fn clawback_spark_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    requested: i64,
    kind: &str,
    idem: &str,
) -> DomainResult<i64> {
    if requested <= 0 {
        return Err(DomainError::Internal(anyhow::anyhow!(
            "clawback 金额必须为正（负数请走扣款原语）: {kind} {requested}"
        )));
    }
    // 先锁人再判：与 earn/spend 同序，防并发双收
    let balance: Option<i64> = sqlx::query_scalar(
        "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(balance) = balance else {
        tracing::warn!(
            user = user_id,
            requested,
            kind,
            "火花收回：账号不存在，本笔跳过"
        );
        return Ok(0);
    };
    let applied = requested.min(balance.max(0));
    if applied == 0 {
        tracing::warn!(
            user = user_id,
            requested,
            balance,
            kind,
            "火花收回被地板夹零：本笔未收回（不把账面写成负数）"
        );
        return Ok(0);
    }
    // 幂等检查在行锁之后（与 earn_spark 同口径），查询失败必须报错
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
    )
    .bind(idem)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if exists {
        return Ok(0);
    }
    let after = checked_balance(balance, -applied)?;
    sqlx::query(
        "INSERT INTO spark_ledger (id, user_id, amount, kind, \
         idempotency_key, balance_after) VALUES \
         (nextval('spark_ledger_id_seq'), $1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(-applied)
    .bind(kind)
    .bind(idem)
    .bind(after)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE users SET spark_balance = spark_balance - $2 WHERE id = $1",
    )
    .bind(user_id)
    .bind(applied)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if applied < requested {
        tracing::warn!(
            user = user_id,
            requested,
            applied,
            balance,
            kind,
            "火花收回部分生效：余额不足，未收满"
        );
    }
    Ok(applied)
}

/// autocommit 版：删帖/删主题这类「一步动作 + 收回」不必把整个 HTTP 处理搬进事务。
pub async fn clawback_spark(
    db: &PgPool,
    user_id: i64,
    requested: i64,
    kind: &str,
    idem: &str,
) -> DomainResult<i64> {
    let mut tx = db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let applied =
        clawback_spark_tx(&mut tx, user_id, requested, kind, idem).await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(applied)
}
