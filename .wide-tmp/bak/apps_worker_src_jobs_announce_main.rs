//! announce 主消费 + process_event 计费。
//! 从 jobs.rs 按域拆出。

use super::process_event::process_event;
use sqlx::PgPool;

/// 消费 announce 事件流（§5.4 链路 ④-⑦）：Redis Stream → 计费流水 + snatches。
/// 事件为 JSON 文本，字段 user/hash/up/down/event/left。
pub async fn consume_announce(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
) -> anyhow::Result<u64> {
    let redis_dead_letter = redis.clone();
    use redis::AsyncCommands;

    // 游标消费：从上次处理到的 ID 继续拉取（Redis 键持久化游标，重启不丢事件、不重复计费）
    let last_id: Option<String> =
        redis.get("flux:announce:cursor").await.unwrap_or(None);
    let from = last_id.clone().unwrap_or_else(|| "-".to_string());

    // 保种时长累计容忍窗 = 2 × announce_interval（与 tracker 下发口径一致，站点设定缺省 1800）。
    // 客户端按 interval 汇报，相邻两次做种 announce 的时间差即真实做种时长；
    // 超窗（离线/故障）不计，防挂机伪造做种时长。
    let announce_interval: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'announce_interval'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .map(|v| v.clamp(60, 86400))
    .unwrap_or(1800);
    let seed_cap = (announce_interval * 2).clamp(3600, 172_800);

    // XRANGE → StreamRangeReply（redis 0.27 类型映射；错误必须可见，不允许静默空消费）
    let reply = match redis
        .xrange::<_, _, _, redis::streams::StreamRangeReply>(
            "flux:announce",
            &from,
            "+",
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(?e, "xrange flux:announce 失败");
            return Err(e.into());
        }
    };

    let mut applied = 0u64;
    let mut last_seen_id: Option<String> = None;
    // P0-1 快照增量化：只刷本轮涉及的用户/种子（此前每轮对 users/torrents 全表重算，
    // 万级种子下每分钟两次全表聚合；纠偏由 run_all 的 reconcile_snapshots 周期兜底）
    let mut touched_users: std::collections::BTreeSet<i64> = Default::default();
    let mut touched_torrents: std::collections::BTreeSet<i64> =
        Default::default();
    for entry in reply.ids {
        let id = entry.id;
        // 已处理过的游标本条跳过
        if Some(&id) == last_id.as_ref() {
            last_seen_id = Some(id);
            continue;
        }
        let Some(payload) = entry
            .map
            .get("payload")
            .and_then(|v| redis::from_redis_value::<String>(v).ok())
        else {
            // 损坏事件进死信，不阻塞游标
            tracing::warn!(%id, "announce 事件损坏，进死信");
            let mut conn_dl = redis_dead_letter.clone();
            let _: Result<(), _> = redis::cmd("RPUSH")
                .arg("flux:announce:dlq")
                .arg(&id)
                .query_async(&mut conn_dl)
                .await;
            last_seen_id = Some(id);
            continue;
        };
        let ev = match serde_json::from_str::<AnnounceEvent>(&payload) {
            Ok(ev) => ev,
            Err(e) => {
                tracing::warn!(%id, ?e, "事件 JSON 解析失败，进死信");
                let mut conn_dl = redis_dead_letter.clone();
                let _: Result<(), _> = redis::cmd("RPUSH")
                    .arg("flux:announce:dlq")
                    .arg(&payload)
                    .query_async(&mut conn_dl)
                    .await;
                last_seen_id = Some(id);
                continue;
            }
        };
        match process_event(db, &ev, seed_cap).await {
            Ok(Some(torrent_id)) => {
                applied += 1;
                let _ = redis::cmd("DEL")
                    .arg(format!("flux:announce:fail:{id}"))
                    .query_async::<()>(&mut redis.clone())
                    .await;
                last_seen_id = Some(id);
                touched_users.insert(ev.user);
                touched_torrents.insert(torrent_id);
                // connectable 抽样联动（0071 P1-9）：不可达 + 零上传 → 疑似假保种，记入作弊探测
                if ev.conn == Some(0) {
                    let _ = sqlx::query(
                        "INSERT INTO cheat_events (user_id, agent, peer_ip, reason) \
                         VALUES ($1, 'connectable', $2, 'suspect_ghost_seed') \
                         ON CONFLICT (user_id, agent, reason) DO UPDATE \
                           SET hits = cheat_events.hits + 1, last_seen = now()",
                    )
                    .bind(ev.user)
                    .bind(&ev.ip)
                    .execute(db)
                    .await;
                }
            }
            Ok(None) => {
                // 种子不存在：跳过计费但推进游标
                applied += 1;
                last_seen_id = Some(id);
            }
            Err(e) => {
                // 处理失败：不推进游标，下轮重试（避免丢失计费）。
                // 审计修复（P0，v2）：持久性错误会永久卡死整条流。原实现的连续失败计数是
                // 函数局部变量——consume_announce 每分钟被独立调用一次，计数每轮归零，
                // 熔断永远触发不了。改为 Redis 键 flux:announce:fail:{id} 持久化计数：
                // 同一 ID 连续失败 6 次进死信并推进游标，事件本体留在 DLQ 供人工补偿计费。
                let fail_key = format!("flux:announce:fail:{id}");
                let streak: i64 = redis::cmd("INCR")
                    .arg(&fail_key)
                    .query_async::<i64>(&mut redis.clone())
                    .await
                    .unwrap_or(1);
                let _ = redis::cmd("EXPIRE")
                    .arg(&fail_key)
                    .arg(3600)
                    .query_async::<()>(&mut redis.clone())
                    .await;
                tracing::error!(%id, ?e, streak, "事件计费失败，游标暂停等待重试");
                if streak >= 6 {
                    tracing::error!(%id, "同一事件连续 6 次失败，转入死信队列并跳过（flux:announce:dlq）");
                    let mut conn_dl = redis_dead_letter.clone();
                    let payload_txt = payload.clone();
                    let _: Result<(), _> = redis::cmd("RPUSH")
                        .arg("flux:announce:dlq")
                        .arg(format!("{id}	{payload_txt}"))
                        .query_async(&mut conn_dl)
                        .await;
                    let _ = redis::cmd("DEL")
                        .arg(&fail_key)
                        .query_async::<()>(&mut redis.clone())
                        .await;
                    applied += 1;
                    last_seen_id = Some(id);
                    continue;
                }
                break;
            }
        }
    }
    if let Some(id) = last_seen_id {
        let cur: Result<(), redis::RedisError> =
            redis.set("flux:announce:cursor", &id).await;
        if let Err(e) = cur {
            // 游标写入失败必须显式报错：静默失败会导致下轮重复计费
            tracing::error!(?e, "游标写入失败（下轮可能重复计费，需人工核对）");
            return Err(e.into());
        }
        let _: () = redis
            .xtrim("flux:announce", redis::streams::StreamMaxlen::Approx(10000))
            .await
            .unwrap_or(());
    }
    // P0-1：快照点刷——仅本轮有事件的用户/种子（权威在流水，快照仅展示）
    let users: Vec<i64> = touched_users.into_iter().collect();
    if !users.is_empty() {
        sqlx::query(
            "UPDATE users SET \
             uploaded = COALESCE((SELECT sum(delta_up) FROM traffic_ledger WHERE user_id = users.id), 0), \
             downloaded = COALESCE((SELECT sum(delta_down) FROM traffic_ledger WHERE user_id = users.id), 0) \
             WHERE id = ANY($1)",
        )
        .bind(&users)
        .execute(db)
        .await?;
    }
    let torrents: Vec<i64> = touched_torrents.into_iter().collect();
    if !torrents.is_empty() {
        // 回填种子做种/下载计数（详情页与保种规则数据源）
        sqlx::query(
            "UPDATE torrents t SET \
             seeders = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.seeding), 0), \
             leechers = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.leeching), 0), \
             times_completed = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.completed_at IS NOT NULL), 0) \
             WHERE t.id = ANY($1)",
        )
        .bind(&torrents)
        .execute(db)
        .await?;
    }
    Ok(applied)
}

#[derive(serde::Deserialize)]
pub(crate) struct AnnounceEvent {
    pub(crate) user: i64,
    pub(crate) hash: String,
    pub(crate) up: i64,
    pub(crate) down: i64,
    #[serde(default)]
    pub(crate) event: String,
    #[serde(default)]
    pub(crate) left: i64,
    /// announce 来源 IP（0071 反作弊：账号 IP 跳变/多 IP 分析）
    #[serde(default)]
    pub(crate) ip: String,
    /// tracker 主动回连抽样结果（0071）：-1=未测（缺省/旧事件） 0=不可达 1=可达
    #[serde(default)]
    pub(crate) conn: Option<i16>,
    /// tracker 收到 announce 的时点（RFC3339）。促销按「事件时点」而非「消费时点」裁决：
    /// 事件积压（DLQ 重试/worker 停机）时避免免费窗口结束后按原价补计费。
    #[serde(default)]
    pub(crate) ts: Option<chrono::DateTime<chrono::Utc>>,
    /// BT 客户端 UA（0098 下载列表「客户端」列）；截断 200
    #[serde(default)]
    pub(crate) agent: String,
}
