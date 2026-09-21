//! announce 消费：agent 块 + 事件计费。
//! 从 jobs.rs 按域拆出。

use sqlx::PgPool;

/// 消费 agent_rules 命中事件（tracker emit_agent_block 投递）→ cheat_events 落库。
/// 同 (user_id, agent, reason) 累加 hits；首次命中投一条管理组信箱（staffmessages）。
pub async fn consume_agent_blocks(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
) -> anyhow::Result<u64> {
    use redis::AsyncCommands;
    // 显式 turbofish：edition 2024 下 redis.get 的 never-type fallback 会拒绝 !: FromRedisValue
    let last_id: Option<String> = redis
        .get::<_, Option<String>>("flux:agentblock:cursor")
        .await
        .unwrap_or(None);
    let from = last_id.clone().unwrap_or_else(|| "-".to_string());
    let reply = match redis
        .xrange::<_, _, _, redis::streams::StreamRangeReply>(
            "flux:agent_block",
            &from,
            "+",
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(?e, "xrange flux:agent_block 失败");
            return Err(e.into());
        }
    };
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
    let mut applied = 0u64;
    let mut last_seen_id: Option<String> = None;
    for entry in reply.ids {
        let id = entry.id;
        if Some(&id) == last_id.as_ref() {
            last_seen_id = Some(id);
            continue;
        }
        let Some(payload) = entry
            .map
            .get("payload")
            .and_then(|v| redis::from_redis_value::<String>(v).ok())
        else {
            last_seen_id = Some(id);
            continue;
        };
        let Ok(ev) = serde_json::from_str::<AgentBlockEvent>(&payload) else {
            tracing::warn!(%id, "agent_block 事件解析失败，跳过");
            last_seen_id = Some(id);
            continue;
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
            "INSERT INTO cheat_events (user_id, agent, peer_ip, reason) VALUES ($1, $2, $3, $4) \
             ON CONFLICT (user_id, agent, reason) DO UPDATE SET hits = cheat_events.hits + 1, last_seen = now()",
        )
        .bind(ev.user)
        .bind(&agent)
        .bind(&ev.ip)
        .bind(&reason)
        .execute(db)
        .await;
        match res {
            Ok(_) => {
                applied += 1;
                last_seen_id = Some(id);
                if !existed {
                    // 首次命中 → 按 agent_hit_action 分级（U5 §12.3）：
                    // log=仅 staffmessages 告警（现状 T3）/ warn=告警+用户警告信
                    let action: String = sqlx::query_scalar::<_, String>(
                        "SELECT value FROM \
                         site_settings WHERE name = 'agent_hit_action'",
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
                        "INSERT INTO staffmessages (user_id, subject, body, permission) \
                         VALUES ($1, '客户端黑白名单自动告警', $2, 'cheater')",
                    )
                    .bind(ev.user)
                    .bind(body)
                    .execute(db)
                    .await;
                    if action == "warn" {
                        let _: Result<_, _> = sqlx::query(
                            "INSERT INTO messages (sender_id, receiver_id, subject, body) \
                             VALUES (NULL, $1, '客户端不合规警告', $2)",
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
            }
            Err(e) => {
                tracing::error!(%id, ?e, "cheat_events 写入失败，游标暂停等待重试");
                break;
            }
        }
    }
    if let Some(id) = last_seen_id {
        let cur: Result<(), redis::RedisError> =
            redis.set("flux:agentblock:cursor", &id).await;
        if let Err(e) = cur {
            tracing::error!(?e, "agentblock 游标写入失败（下轮可能重复计数）");
            return Err(e.into());
        }
        let _: () = redis
            .xtrim(
                "flux:agent_block",
                redis::streams::StreamMaxlen::Approx(5000),
            )
            .await
            .unwrap_or(());
    }
    Ok(applied)
}
