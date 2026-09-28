//! announce 主消费 + process_event 计费。
//! 从 jobs.rs 按域拆出。
//!
//! 0225 G30-B2：全局游标 XRANGE → 消费者组 XREADGROUP（组内投递唯一，
//! 多 worker 真分摊）；失败语义不变——不 ACK 留 PEL 重试，连续 6 次进
//! DLQ 后 ACK；旧游标兼容迁移见 group.rs::ensure_group。

use super::group::{self, ANNOUNCE_GROUP};
use super::process_event::process_event;
use sqlx::PgPool;
use std::collections::BTreeSet;

/// 消费 announce 事件流（§5.4 链路 ④-⑦）：Redis Stream → 计费流水 + snatches。
/// 事件为 JSON 文本，字段 user/hash/up/down/event/left。
pub async fn consume_announce(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
) -> anyhow::Result<u64> {
    use redis::AsyncCommands;

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

    let consumer = group::consumer_name();
    let mut touched_users: BTreeSet<i64> = BTreeSet::new();
    let mut touched_torrents: BTreeSet<i64> = BTreeSet::new();
    let mut applied = 0u64;

    #[allow(clippy::too_many_arguments)]
    async fn handle_entry(
        db: &PgPool,
        redis: &mut redis::aio::ConnectionManager,
        id: &str,
        payload: &str,
        seed_cap: i64,
        touched_users: &mut BTreeSet<i64>,
        touched_torrents: &mut BTreeSet<i64>,
    ) -> anyhow::Result<bool> {
        // Ok(true) = ACK
        let ev = match serde_json::from_str::<AnnounceEvent>(payload) {
            Ok(ev) => ev,
            Err(e) => {
                tracing::warn!(%id, ?e, "事件 JSON 解析失败，进死信");
                ANNOUNCE_GROUP.dlq_push_with(redis, payload).await;
                return Ok(true);
            }
        };
        match process_event(db, &ev, seed_cap).await {
            Ok(Some(torrent_id)) => {
                let _ = redis::cmd("DEL")
                    .arg(format!("flux:announce:fail:{id}"))
                    .query_async::<()>(redis)
                    .await;
                touched_users.insert(ev.user);
                touched_torrents.insert(torrent_id);
                // connectable 抽样联动（0071 P1-9）：不可达 + 零上传 →
                // 疑似假保种，记入作弊探测
                if ev.conn == Some(0) {
                    let _ = sqlx::query(
                        "INSERT INTO cheat_events (user_id, agent, \
                         peer_ip, reason) VALUES ($1, 'connectable', \
                         $2, 'suspect_ghost_seed') \
                         ON CONFLICT (user_id, agent, reason) DO UPDATE \
                           SET hits = cheat_events.hits + 1, \
                               last_seen = now()",
                    )
                    .bind(ev.user)
                    .bind(&ev.ip)
                    .execute(db)
                    .await;
                }
                Ok(true)
            }
            Ok(None) => Ok(true), // 种子不存在：跳过计费
            Err(e) => {
                // 处理失败：不 ACK 留 PEL 重试（避免丢失计费）。
                // 持久化失败计数（P0 v2）：同一 ID 连续失败 6 次进死信
                // 并 ACK，事件本体留在 DLQ 供人工补偿计费。
                let fail_key = format!("flux:announce:fail:{id}");
                let streak: i64 = redis::cmd("INCR")
                    .arg(&fail_key)
                    .query_async::<i64>(redis)
                    .await
                    .unwrap_or(1);
                let _ = redis::cmd("EXPIRE")
                    .arg(&fail_key)
                    .arg(3600)
                    .query_async::<()>(redis)
                    .await;
                tracing::error!(%id, ?e, streak, "事件计费失败，留 PEL 等待重试");
                if streak >= 6 {
                    tracing::error!(%id, "同一事件连续 6 次失败，转死信并跳过");
                    ANNOUNCE_GROUP
                        .dlq_push_with(redis, &format!("{id}\t{payload}"))
                        .await;
                    let _ = redis::cmd("DEL")
                        .arg(&fail_key)
                        .query_async::<()>(redis)
                        .await;
                    return Ok(true);
                }
                Ok(false)
            }
        }
    }

    // 新消息
    for (id, payload) in
        group::read_group(redis, &ANNOUNCE_GROUP, &consumer).await
    {
        if payload.is_empty() {
            ANNOUNCE_GROUP
                .dlq_push_with(redis, &format!("{id}\t(no payload)"))
                .await;
            ANNOUNCE_GROUP.ack(redis, &id).await;
            applied += 1;
            continue;
        }
        if handle_entry(
            db,
            redis,
            &id,
            &payload,
            seed_cap,
            &mut touched_users,
            &mut touched_torrents,
        )
        .await?
        {
            ANNOUNCE_GROUP.ack(redis, &id).await;
            applied += 1;
        }
    }
    // 回收遗孤 pending（worker 崩溃自愈）：空闲 > 120s
    for (id, payload) in
        group::reclaim_stale(redis, &ANNOUNCE_GROUP, &consumer, 360_000).await
    {
        if payload.is_empty() {
            continue;
        }
        if handle_entry(
            db,
            redis,
            &id,
            &payload,
            seed_cap,
            &mut touched_users,
            &mut touched_torrents,
        )
        .await?
        {
            ANNOUNCE_GROUP.ack(redis, &id).await;
            applied += 1;
        }
    }
    // XTRIM 修剪（与旧机制同口径，只裁长度不动组）
    let _: () = redis
        .xtrim("flux:announce", redis::streams::StreamMaxlen::Approx(10000))
        .await
        .unwrap_or(());

    // P0-1：快照点刷——仅本轮有事件的用户/种子（权威在流水，快照仅展示）
    if !touched_users.is_empty() {
        let users: Vec<i64> = touched_users.into_iter().collect();
        sqlx::query(
            "UPDATE users SET \
             uploaded = COALESCE((SELECT base_up FROM balance_baseline \
                 WHERE user_id = users.id), 0) \
                 + COALESCE((SELECT sum(delta_up) FROM traffic_ledger \
                 WHERE user_id = users.id), 0), \
             downloaded = COALESCE((SELECT base_down FROM \
                 balance_baseline WHERE user_id = users.id), 0) \
                 + COALESCE((SELECT sum(delta_down) FROM \
                 traffic_ledger WHERE user_id = users.id), 0) \
             WHERE id = ANY($1)",
        )
        .bind(&users)
        .execute(db)
        .await?;
    }
    if !touched_torrents.is_empty() {
        let torrents: Vec<i64> = touched_torrents.into_iter().collect();
        sqlx::query(
            "UPDATE torrents t SET \
             seeders = COALESCE((SELECT count(*) FROM snatches s \
                 WHERE s.torrent_id = t.id AND s.seeding), 0), \
             leechers = COALESCE((SELECT count(*) FROM snatches s \
                 WHERE s.torrent_id = t.id AND s.leeching), 0), \
             times_completed = COALESCE((SELECT count(*) FROM snatches s \
                 WHERE s.torrent_id = t.id \
                   AND s.completed_at IS NOT NULL), 0) \
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
