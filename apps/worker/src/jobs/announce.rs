//! announce 消费：agent 块 + 事件计费。
//! 从 jobs.rs 按域拆出。
//!
//! 0225 G30-B2：全局游标 XRANGE → 消费者组 XREADGROUP（见 group.rs）；
//! 失败留 PEL 重试语义与旧「游标暂停」等价。

use super::group::{self, AGENTBLOCK_GROUP};
use sqlx::PgPool;

/// 消费 agent_rules 命中事件（tracker emit_agent_block 投递）→ cheat_events 落库。
/// 同 (user_id, agent, reason) 累加 hits；首次命中投一条管理组信箱（staffmessages）。
pub async fn consume_agent_blocks(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
) -> anyhow::Result<u64> {
    use redis::AsyncCommands;
    #[derive(serde::Deserialize)]
    struct AgentBlockEvent {
        user: i64,
        #[serde(default)]
        agent: String,
        #[serde(default)]
        ip: String,
        #[serde(default)]
        reason: String,
    }
    let consumer = group::consumer_name();
    let mut applied = 0u64;

async fn handle_entry(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
    id: &str,
    payload: &str,
) -> anyhow::Result<bool> {
        let Ok(ev) = serde_json::from_str::<AgentBlockEvent>(payload) else {
            tracing::warn!(%id, "agent_block 事件解析失败，进死信");
            AGENTBLOCK_GROUP.dlq_push_with(redis, payload).await;
            return Ok(true);
        };
        let agent = if ev.agent.len() > 128 {
            ev.agent[..128].to_string()
        } else {
            ev.agent
        };
        let reason = if ev.reason.len() > 200 {
            ev.reason[..200].to_string()
        } else {
            ev.reason
        };
        // 首次命中判定先于写入（tracker 侧已 1h 去重，这里的额外查询可忽略不计）
        let existed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM cheat_events WHERE \
             user_id = $1 AND agent = $2 AND reason = $3)",
        )
        .bind(ev.user)
        .bind(&agent)
        .bind(&reason)
        .fetch_one(db)
        .await
        .unwrap_or(true);
        let res = sqlx::query(
            "INSERT INTO cheat_events (user_id, agent, peer_ip, reason) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (user_id, agent, reason) DO UPDATE \
               SET hits = cheat_events.hits + 1, last_seen = now()",
        )
        .bind(ev.user)
        .bind(&agent)
        .bind(&ev.ip)
        .bind(&reason)
        .execute(db)
        .await;
        match res {
            Ok(_) => {
                if !existed {
                    // 首次命中 → 按 agent_hit_action 分级（U5 §12.3）：
                    // log=仅 staffmessages 告警（现状 T3）/ warn=告警+用户警告信
                    let action: String =
                        sqlx::query_scalar::<_, String>(
                            "SELECT value FROM site_settings \
                             WHERE name = 'agent_hit_action'",
                        )
                        .fetch_optional(db)
                        .await
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| "log".into());
                    let body = format!(
                        "用户 #{} 的客户端命中黑白名单规则，tracker 已拒绝其 announce。\n客户端：{}\nIP：{}\n原因：{}\n（本条为系统自动告警，累计情况见后台「作弊探测」）",
                        ev.user,
                        if agent.is_empty() { "(空)" } else { &agent },
                        if ev.ip.is_empty() { "(未知)" } else { &ev.ip },
                        reason,
                    );
                    let _: Result<_, _> = sqlx::query(
                        "INSERT INTO staffmessages (user_id, subject, body, \
                         permission) VALUES ($1, '客户端黑白名单自动告警', \
                         $2, 'cheater')",
                    )
                    .bind(ev.user)
                    .bind(body)
                    .execute(db)
                    .await;
                    if action == "warn" {
                        let _: Result<_, _> = sqlx::query(
                            "INSERT INTO messages (sender_id, receiver_id, \
                             subject, body) VALUES (NULL, $1, \
                             '客户端不合规警告', $2)",
                        )
                        .bind(ev.user)
                        .bind(format!(
                            "您使用的客户端（{}）不符合本站允许名单（原因：{}）。请更换为允许的客户端，否则将无法继续做种/下载。如有疑问请联系管理组。",
                            if agent.is_empty() { "(空)" } else { &agent },
                            reason,
                        ))
                        .execute(db)
                        .await;
                    }
                }
                Ok(true)
            }
            Err(e) => {
                tracing::error!(%id, ?e, "cheat_events 写入失败，留 PEL 重试");
                Ok(false)
            }
        }
    }

    for (id, payload) in group::read_group(redis, &AGENTBLOCK_GROUP, &consumer)
        .await
    {
        if payload.is_empty() {
            AGENTBLOCK_GROUP.dlq_push_with(redis, &format!("{id}\t(no payload)")).await;
            AGENTBLOCK_GROUP.ack(redis, &id).await;
            applied += 1;
            continue;
        }
        if handle_entry(db, redis, &id, &payload).await? {
            AGENTBLOCK_GROUP.ack(redis, &id).await;
            applied += 1;
        }
    }
    for (id, payload) in group::reclaim_stale(
        redis, &AGENTBLOCK_GROUP, &consumer, 120_000,
    ).await {
        if payload.is_empty() {
            continue;
        }
        if handle_entry(db, redis, &id, &payload).await? {
            AGENTBLOCK_GROUP.ack(redis, &id).await;
            applied += 1;
        }
    }
    let _: () = redis
        .xtrim(
            "flux:agent_block",
            redis::streams::StreamMaxlen::Approx(5000),
        )
        .await
        .unwrap_or(());
    Ok(applied)
}
