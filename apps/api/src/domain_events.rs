//! 全域事件 outbox（0374）：事务写入 + 幂等消费 + 死信。
//!
//! 设计取舍（先说清「不做什么」）：
//! - **不做** Kafka/Redis Stream 那套外部中间件。单体站的事件量级用不
//!   到，而外部中间件会把「投递可靠性」换成「多一个会挂的组件」。
//! - **不做**每事件独立表。事件形状差异用 `payload` JSONB 承载，字段
//!   由写入口自己保证（与仓内 `audit_detail` 同一口径）。
//! - **做**三件 PT Studio 领先、而本仓此前完全没有的：业务事务内写入
//!   （业务回滚事件也回滚）、幂等消费（dedupe_key）、重试与死信
//!   （attempts 超限不再无限重投，留 last_error 供扫查）。
//!
//! 使用纪律：
//! - `emit` 要**在业务事务里**调（`&mut Transaction`），不要拿 pool 另写
//!   ——另写就退化成 0283 那种「业务失败事件还在跑」的裂缝。
//! - 消费方按 `event_subscriptions` 登记，新增消费方只加一行，不改
//!   业务代码；`dispatch` 是这条总线的执行面。
//! - 事件名常量集中在本模块 `pub mod events`，不要在调用点写字面量。

use serde::Serialize;
use sqlx::{Postgres, Transaction};

use crate::errors::{DomainError, DomainResult};

/// 事件名常量。事件名是契约的一部分（订阅表按它匹配），
/// 改名要同时改订阅行——所以一律走常量。
pub mod events {
    /// 用户登录成功（`DomainEvent::UserLogin` 的总线侧同名）
    pub const USER_LOGIN: &str = "user.login";
    /// 种子发布成功（已过审核入列表）
    pub const TORRENT_UPLOADED: &str = "torrent.uploaded";
    /// 做种里程碑（小时数）
    pub const SEEDING_MILESTONE: &str = "seeding.milestone";
}

/// 一次消费后的处置结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dispatched {
    /// 消费成功，标记已消费。
    Ok,
    /// 消费失败但仍可重试（attempts+1，达上限转死信）。
    Retry,
    /// 明确不可重试（如载荷形状不对）——直接进死信，不浪费重试预算。
    Dead,
}

/// 重试上限。超过即视为死信（`consumed_at` 仍 NULL + `last_error` 有值）。
pub const MAX_ATTEMPTS: i32 = 5;

/// 在**业务事务内**写入一条事件。
///
/// 与业务写同生共死：业务回滚，事件也回滚。这是 outbox 相对
/// 「业务提交后再 emit」的核心差别——后者在进程崩溃的缝里会丢事件，
/// 或者业务失败事件还在跑。
pub async fn emit_tx(
    tx: &mut Transaction<'_, Postgres>,
    event: &str,
    aggregate: Option<&str>,
    aggregate_id: Option<i64>,
    payload: &serde_json::Value,
    dedupe_key: Option<&str>,
) -> DomainResult<()> {
    let r = sqlx::query(
        "INSERT INTO domain_events \
           (event, aggregate, aggregate_id, payload, dedupe_key) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (dedupe_key) WHERE dedupe_key IS NOT NULL \
           AND consumed_at IS NULL DO NOTHING",
    )
    .bind(event)
    .bind(aggregate)
    .bind(aggregate_id)
    .bind(payload)
    .bind(dedupe_key)
    .execute(&mut **tx)
    .await;
    match r {
        Ok(_) => Ok(()),
        // 23505 = unique_violation。幂等键撞车是**预期**路径（同键未消费过），
        // 返回 Ok 让业务继续——重复事件不是错误。
        Err(sqlx::Error::Database(e))
            if e.code().as_deref() == Some("23505") =>
        {
            Ok(())
        }
        Err(e) => Err(DomainError::Internal(e.into())),
    }
}

/// 在**业务事务内**写入一条事件，载荷由 `T: Serialize` 序列化。
pub async fn emit_typed<T: Serialize>(
    tx: &mut Transaction<'_, Postgres>,
    event: &str,
    aggregate: Option<&str>,
    aggregate_id: Option<i64>,
    payload: &T,
    dedupe_key: Option<&str>,
) -> DomainResult<()> {
    let v = serde_json::to_value(payload)
        .map_err(|e| DomainError::Internal(e.into()))?;
    emit_tx(tx, event, aggregate, aggregate_id, &v, dedupe_key).await
}

/// 待消费事件行。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PendingEvent {
    pub id: i64,
    pub event: String,
    pub aggregate: Option<String>,
    pub aggregate_id: Option<i64>,
    pub payload: serde_json::Value,
    pub attempts: i32,
}

/// 拉取一批待消费事件（按 id 顺序，热路径走部分索引）。
///
/// 只拉 `attempts < MAX_ATTEMPTS` 的行：已达上限的进死信，不再反复
/// 出现在消费循环里拖慢正常事件。
pub async fn fetch_pending(
    pool: &sqlx::PgPool,
    limit: i64,
) -> DomainResult<Vec<PendingEvent>> {
    let rows = sqlx::query_as::<_, PendingEvent>(
        "SELECT id, event, aggregate, aggregate_id, payload, attempts \
           FROM domain_events \
          WHERE consumed_at IS NULL AND attempts < $1 \
          ORDER BY id LIMIT $2",
    )
    .bind(MAX_ATTEMPTS)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(rows)
}

/// 某事件名是否有活跃订阅。**无订阅直接跳过投递**——事件表是全站
/// 写入的热路径，没有订阅的事件（比如调试期新加的）不必惊动消费方。
pub async fn has_active_subscription(
    pool: &sqlx::PgPool,
    event: &str,
) -> DomainResult<bool> {
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM event_subscriptions \
          WHERE event = $1 AND active",
    )
    .bind(event)
    .fetch_one(pool)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(n > 0)
}

/// 标记已消费（投递成功）。
pub async fn mark_consumed(pool: &sqlx::PgPool, id: i64) -> DomainResult<()> {
    sqlx::query(
        "UPDATE domain_events SET consumed_at = now() WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 标记一次失败：`attempts` +1 并记 `last_error`。
/// `attempts` 达 `MAX_ATTEMPTS` 后自然不再被 `fetch_pending` 捞出，
/// 即成死信（`consumed_at` 仍 NULL，可扫查）。
pub async fn mark_failed(
    pool: &sqlx::PgPool,
    id: i64,
    reason: &str,
) -> DomainResult<()> {
    let reason: String = reason.chars().take(500).collect();
    sqlx::query(
        "UPDATE domain_events SET attempts = attempts + 1, last_error = $2 \
          WHERE id = $1",
    )
    .bind(id)
    .bind(&reason)
    .execute(pool)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 死信行数（按事件名聚类）。运维/后台可观测面的入口。
pub async fn deadletter_counts(
    pool: &sqlx::PgPool,
) -> DomainResult<Vec<(String, i64)>> {
    let rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT event, count(*) FROM domain_events \
          WHERE consumed_at IS NULL AND attempts >= $1 \
          GROUP BY event ORDER BY count(*) DESC",
    )
    .bind(MAX_ATTEMPTS)
    .fetch_all(pool)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(rows)
}

/// 手工重投一条死信（把 attempts 归零）。后台「重试」按钮的落点。
pub async fn retry_deadletter(
    pool: &sqlx::PgPool,
    id: i64,
) -> DomainResult<bool> {
    let r = sqlx::query(
        "UPDATE domain_events SET attempts = 0, last_error = NULL \
          WHERE id = $1 AND consumed_at IS NULL",
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(r.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_names_are_namespaced() {
        // 事件名会写进订阅表，裸词容易撞（"login" vs "user.login"）。
        for n in [
            events::USER_LOGIN,
            events::TORRENT_UPLOADED,
            events::SEEDING_MILESTONE,
        ] {
            assert!(n.contains('.'), "事件名必须带命名空间：{n}");
        }
    }

    #[test]
    fn retry_budget_is_bounded() {
        assert!(MAX_ATTEMPTS > 0);
        // 死信判据依赖 attempts 单调 +1，所以上限本身不能是 0/1
        // （1 意味着第一次失败即死信，等于没有重试）。
        assert!(MAX_ATTEMPTS >= 2);
    }

    #[test]
    fn dispatched_variants_are_distinct() {
        assert_ne!(Dispatched::Ok, Dispatched::Retry);
        assert_ne!(Dispatched::Retry, Dispatched::Dead);
        assert_ne!(Dispatched::Ok, Dispatched::Dead);
    }
}
