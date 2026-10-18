//! 事件 outbox 的消费面（0374）：常驻循环 + 订阅 fan-out + 死信处置。
//
//! 与 domain_events.rs（写入面 + 存储/查询）分家：那个文件回答
//! 「有哪些事件、怎么入队」，这个文件回答「谁来消费、失败了怎么办」。

use sqlx::PgPool;

use crate::domain_events::{
    self, fetch_pending, has_active_subscription, mark_consumed, mark_failed,
    Dispatched, PendingEvent, MAX_ATTEMPTS,
};
use crate::errors::{DomainError, DomainResult};

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
        let subscribed = match has_active_subscription(pool, &ev.event).await {
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
async fn dispatch(pool: &sqlx::PgPool, ev: &PendingEvent) -> Dispatched {
    use crate::domain_events::events;
    match ev.event.as_str() {
        events::TORRENT_UPLOADED => {
            let tid = ev.aggregate_id.or_else(|| {
                ev.payload.get("torrent_id").and_then(|v| v.as_i64())
            });
            let owner = ev
                .payload
                .get("owner_id")
                .and_then(|v| v.as_i64())
                .unwrap_or_default();
            match tid {
                None => {
                    tracing::warn!(
                        id = ev.id,
                        "torrent.uploaded 缺 torrent_id"
                    );
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
