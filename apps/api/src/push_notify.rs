//! Web Push 事件推送共享模块（0283 P0-1）：订阅按 topic 过滤 + 批量投递 + 失效清理。
//!
//! 背景：push_http 只有订阅管理四端点，全站无任何业务事件调用投递——Web Push
//! 此前是「只收不发」的空壳。本模块补发送侧，三类事件接入：
//!   - 私信（api message_send，即时）
//!   - H&R 预警/违规（worker hr job，跟随 prewarn/enforce 轮次）
//!   - 愿望单命中（worker wishlist_notify，小时级聚合）
//!
//! topic 口径与订阅端 VALID_TOPICS 对齐：hr / promo / message。
//! 用户级开关沿用 notice_prefs JSONB（push_hr / push_promo / push_message，
//! 缺省 true）——与站内信通知偏好同机制，notice.rs 白名单已同步登记。

use sqlx::PgPool;

use crate::push_http::crypto::{deliver, vapid_from_env, PushResult};

/// topic → notice_prefs 开关键
fn pref_key(topic: &str) -> &'static str {
    match topic {
        "hr" => "push_hr",
        "promo" => "push_promo",
        _ => "push_message",
    }
}

/// 合法 topic 集合（与订阅端 VALID_TOPICS 对齐；outbox 消费侧校验）
const TOPICS: &[&str] = &["hr", "promo", "message"];

#[derive(sqlx::FromRow)]
struct SubRow {
    id: i64,
    endpoint: String,
    p256dh: String,
    auth: String,
    /// 用户级推送开关（notice_prefs JSONB；true = 用户已关）
    pref_off: Option<bool>,
}

/// 给某用户的所有活跃订阅推送一条事件（topic 过滤 + 偏好过滤 + Gone 清理）。
/// 尽力而为：任何失败只记日志，绝不打断调用方主流程。
pub async fn push_to_user(
    db: &PgPool,
    user_id: i64,
    topic: &str,
    title: &str,
    body: &str,
) {
    let subs: Vec<SubRow> = match sqlx::query_as(
        "SELECT p.id, p.endpoint, p.p256dh, p.auth, \
                COALESCE((u.notice_prefs->>$2)::boolean, false) AS pref_off \
         FROM push_subscriptions p JOIN users u ON u.id = p.user_id \
         WHERE p.user_id = $1 AND p.expired_at IS NULL AND $3 = ANY(p.topics) \
         LIMIT 20",
    )
    .bind(user_id)
    .bind(pref_key(topic))
    .bind(topic)
    .fetch_all(db)
    .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(?e, user_id, topic, "push 订阅查询失败");
            return;
        }
    };
    if subs.is_empty() {
        return;
    }
    let vapid = vapid_from_env();
    if vapid.is_none() {
        return; // 未配置 VAPID：静默跳过（订阅管理不受影响）
    }
    for s in &subs {
        if s.pref_off == Some(true) {
            continue;
        }
        match deliver(&s.endpoint, &s.p256dh, &s.auth, title, body, &vapid)
            .await
        {
            PushResult::Gone => {
                let _ = sqlx::query(
                    "UPDATE push_subscriptions SET expired_at = now() \
                     WHERE id = $1",
                )
                .bind(s.id)
                .execute(db)
                .await;
            }
            _ => {}
        }
    }
}

#[derive(sqlx::FromRow)]
struct OutboxRow {
    id: i64,
    user_id: i64,
    topic: String,
    title: String,
    body: String,
}

/// outbox 消费循环（api 常驻）：每 15s 捞一批未投递行投递。
/// G30 多副本：两轮之间用 advisory lock 短持——多 api 实例并发消费时同一行
/// 至多被锁持有者投递一次；锁仅在捞取+标记窗口内持有（投递本身在锁外重试安全：
/// 行标记 processed 后不再捞，重复投递最坏一条重复通知）。
pub async fn spawn_outbox_consumer(db: PgPool) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        if let Err(e) = outbox_drain(&db).await {
            tracing::warn!(?e, "push outbox drain failed");
        }
    }
}

async fn outbox_drain(db: &PgPool) -> anyhow::Result<()> {
    const LOCK: i64 = 0x70757368; // "push"
    let got: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(LOCK)
        .fetch_one(db)
        .await
        .unwrap_or(false);
    if !got {
        return Ok(());
    }
    let result = outbox_drain_locked(db).await;
    let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(LOCK)
        .execute(db)
        .await;
    result
}

async fn outbox_drain_locked(db: &PgPool) -> anyhow::Result<()> {
    let rows: Vec<OutboxRow> = sqlx::query_as(
        "SELECT id, user_id, topic, title, body FROM push_outbox \
         WHERE processed_at IS NULL AND topic = ANY($1) \
         ORDER BY id LIMIT 100",
    )
    .bind(TOPICS)
    .fetch_all(db)
    .await?;
    if rows.is_empty() {
        return Ok(());
    }
    for r in &rows {
        push_to_user(db, r.user_id, &r.topic, &r.title, &r.body).await;
        // 无论是否有活跃订阅/是否送达，单轮即标记完成（重试语义交给业务事件
        // 重发——dedupe_key 防的是同一事件的重复入队，不是投递重试）
        sqlx::query(
            "UPDATE push_outbox SET processed_at = now(), processed_ok = true \
             WHERE id = $1",
        )
        .bind(r.id)
        .execute(db)
        .await?;
    }
    tracing::debug!(n = rows.len(), "push outbox drained");
    Ok(())
}
