//! announce 单条事件处理（从 announce_main.rs 拆出，300 行门禁）。
//!
//! 2026-11-06：函数签名由「就地改 touched_users/touched_torrents」改为
//! **返回** (ack, user_id, torrent_id)——主消费循环改为有界并发后，
//! 多条事件在各自任务里跑，共享集合没法用 &mut 借出去。

use super::announce_main::AnnounceEvent;
use super::group::ANNOUNCE_GROUP;
use super::process_event::process_event;
use sqlx::PgPool;

/// 处理一条 announce 事件。
///
/// 返回 `(ack, user_id, torrent_id)`：
/// - `ack=true` 表示可以 XACK（已入账 / 已判死信 / 种子不存在）；
/// - `user_id`/`torrent_id` 是本条事件触及的快照点刷对象（可为 None）。
/// 失败返回 Err —— 调用方**不要** ACK，事件留在 PEL 等下一轮重试。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_entry(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
    id: &str,
    payload: &str,
    seed_cap: i64,
) -> anyhow::Result<(bool, Option<i64>, Option<i64>)> {
    // Ok(true) = ACK
    let ev = match serde_json::from_str::<AnnounceEvent>(payload) {
        Ok(ev) => ev,
        Err(e) => {
            tracing::warn!(%id, ?e, "事件 JSON 解析失败，进死信");
            ANNOUNCE_GROUP.dlq_push_with(redis, payload).await;
            return Ok((true, None, None));
        }
    };
    match process_event(db, &ev, seed_cap, id).await {
        Ok(Some(torrent_id)) => {
            let _ = redis::cmd("DEL")
                .arg(format!("flux:announce:fail:{id}"))
                .query_async::<()>(redis)
                .await;
            // 对刷检测日志（0301）：来源三元组落表，供 self_deal_check
            // 做 30 分钟窗口聚合（同 IP 双账号一传一下）。失败仅告警——
            // 日志缺失只降级检测灵敏度，不影响计费主链路。
            if !ev.ip.is_empty() {
                let _ = sqlx::query(
                    "INSERT INTO announce_ips \
                     (user_id, ip, info_hash, seeding, leeching) \
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(ev.user)
                .bind(&ev.ip)
                .bind(&ev.hash)
                .bind(ev.left == 0)
                .bind(ev.left > 0)
                .execute(db)
                .await;
            }
            // connectable 抽样联动（0071 P1-9 → 2026-10-07 保种组审计 P0-2）：
            // 不可达 + 零上传 + 无监听端口 → 幽灵做种实锤。事件键从
            // 'connectable' 改为 'ghost:{hash8}'——速度/回退/幽灵三类统一
            // 按种子分键（旧 'connectable' 是全账号单行，hits 聚合后无法
            // 定位是哪颗种子在刷，累进处置也分不出轻重）。
            if ev.conn == Some(0) && ev.port == 0 && ev.up == 0 {
                let ghost_key =
                    format!("ghost:{}", &ev.hash[..8.min(ev.hash.len())]);
                let _ = sqlx::query(
                    "INSERT INTO cheat_events (user_id, agent, \
                     peer_ip, reason) VALUES ($1, $2, \
                     $3, 'ghost_seed（无监听端口且回连不可达的幽灵做种）') \
                     ON CONFLICT (user_id, agent, reason) DO UPDATE \
                       SET hits = cheat_events.hits + 1, \
                           last_seen = now()",
                )
                .bind(ev.user)
                .bind(&ghost_key)
                .bind(&ev.ip)
                .execute(db)
                .await;
            }
            Ok((true, Some(ev.user), Some(torrent_id)))
        }
        Ok(None) => Ok((true, None, None)), // 种子不存在：跳过计费
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
                return Ok((true, None, None));
            }
            Err(e)
        }
    }
}
