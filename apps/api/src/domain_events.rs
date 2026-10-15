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
        Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("23505") => {
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
pub async fn retry_deadletter(pool: &sqlx::PgPool, id: i64) -> DomainResult<bool> {
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
// ============ 消费循环（0374） ============

/// 消费循环的 advisory lock 键（"fxe1"）。多实例部署时只有一个真正消费，
/// 与 `push_notify` 的锁同一形态——两把不同的锁，两条独立的队列。
const DRAIN_LOCK: i64 = 0x6678_6531;

/// 常驻消费循环。间隔 5s：比 push 的 15s 短，因为总线事件的时效性更高
/// （置顶、通知这类「用户正在等」的动作）。
pub async fn spawn_consumer(pool: sqlx::PgPool) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        if let Err(e) = drain_once(&pool).await {
            tracing::warn!(?e, "domain event drain failed");
        }
    }
}

/// 一轮消费：拿锁 → 拉一批 → 逐条分发 → 记成功/失败 → 放锁。
pub async fn drain_once(pool: &sqlx::PgPool) -> DomainResult<()> {
    let got: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(DRAIN_LOCK)
        .fetch_one(pool)
        .await
        .unwrap_or(false);
    if !got {
        return Ok(());
    }
    let r = drain_locked(pool).await;
    let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(DRAIN_LOCK)
        .execute(pool)
        .await;
    r
}

async fn drain_locked(pool: &sqlx::PgPool) -> DomainResult<()> {
    let batch = fetch_pending(pool, 100).await?;
    for ev in batch {
        // 无活跃订阅直接标消费：这行事件本就不该惊动任何人，留着只会
        // 反复出现在每轮拉取里（热路径成本）。
        let subscribed =
            match has_active_subscription(pool, &ev.event).await {
                Ok(v) => v,
                Err(e) => {
                    let _ = mark_failed(pool, ev.id, &e.to_string()).await;
                    continue;
                }
            };
        if !subscribed {
            let _ = mark_consumed(pool, ev.id).await;
            continue;
        }
        match dispatch(pool, &ev).await {
            Dispatched::Ok => {
                let _ = mark_consumed(pool, ev.id).await;
            }
            Dispatched::Retry => {
                let _ = mark_failed(pool, ev.id, "消费方返回可重试失败").await;
            }
            Dispatched::Dead => {
                // 不可重试：直接把重试预算打满，下轮就不再被 fetch 捞到，
                // last_error 留原因供 deadletter_counts / retry_deadletter。
                let _ = sqlx::query(
                    "UPDATE domain_events SET attempts = $2, last_error = $3 \
                      WHERE id = $1",
                )
                .bind(ev.id)
                .bind(MAX_ATTEMPTS)
                .bind("消费方判定不可重试")
                .execute(pool)
                .await;
            }
        }
    }
    Ok(())
}

/// 分发到已登记的消费方。
///
/// 当前只有一个真实消费方：`plugin.auto_pin_official`（官种自动置顶）。
/// 它把事件载荷翻译成既有 `Plugin` hook 的调用——**不重跑 SQL**：
/// 插件 hook 自己在 `PluginCtx`/`sql_guard` 边界内完成写入与校验，
/// 总线只负责「什么时候该跑」（事件驱动替代调用点手写）。
async fn dispatch(
    pool: &sqlx::PgPool,
    ev: &PendingEvent,
) -> Dispatched {
    use crate::domain_events::events;
    match ev.event.as_str() {
        events::TORRENT_UPLOADED => {
            let tid = ev.aggregate_id.or_else(|| {
                ev.payload.get("torrent_id").and_then(|v| v.as_i64())
            });
            let owner = ev.payload
                .get("owner_id")
                .and_then(|v| v.as_i64())
                .unwrap_or_default();
            match tid {
                None => {
                    tracing::warn!(id = ev.id, "torrent.uploaded 缺 torrent_id");
                    Dispatched::Dead
                }
                Some(tid) => {
                    // 直接用 PluginManager + pool 调 hook，不造 AppState：
                    // 后者要 Redis/JWT，消费循环只需要 DB 与插件表。
                    crate::plugins::PluginManager::builtin()
                        .dispatch_upload_pool(pool, tid, owner);
                    Dispatched::Ok
                }
            }
        }
        _ => {
            // 有订阅但没有实现：不算失败（订阅是站长的登记表，不是
            // 「必须已实现」的断言）。标消费，避免每轮反复捞。
            tracing::debug!(
                event = %ev.event,
                id = ev.id,
                "无消费方实现，按已处理计"
            );
            Dispatched::Ok
        }
    }
}