//! 交叉上报消费（2026-10-07 P0-2 治本，自 announce_main 邻近域新建）。
//!
//! 消费 flux:xreport（tracker 校验过的「leecher 从 peer X 下载了 N 字节」），
//! 按 (uploader, torrent) 累加进 upload_corroborated —— 这是上传量的**可信上界**。
//! 计费侧（process_event）把 uploader 的 credited upload 限制在此界内，
//! 使「上传量全自报」无法做高 ratio。
//!
//! 幂等两层：
//!   ① xreport_seen（Redis 流 entry id）—— 同一 entry 重投/回收不重复累加；
//!   ② 单调累加（bytes = bytes + 本笔），即使 ① 被绕过也不会翻倍回退。

use super::group::{self, XREPORT_GROUP};
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub(crate) struct XReportEvent {
    pub(crate) hash: String,
    pub(crate) leecher: i64,
    /// [[上传者 user_id, 字节数], ...]（tracker 已校验目标 peer 合法且非自己）
    #[serde(default)]
    pub(crate) reports: Vec<(i64, i64)>,
    #[serde(default)]
    pub(crate) ip: String,
}

/// 消费一轮交叉上报，返回本轮佐证累计的条数。
pub async fn consume_xreport(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
) -> anyhow::Result<u64> {
    let consumer = group::consumer_name();
    let mut applied = 0u64;

    async fn handle(
        db: &PgPool,
        id: &str,
        payload: &str,
    ) -> anyhow::Result<bool> {
        let Ok(ev) = serde_json::from_str::<XReportEvent>(payload) else {
            tracing::warn!(%id, "xreport 事件解析失败，丢弃");
            return Ok(true);
        };
        if ev.reports.is_empty() {
            return Ok(true);
        }
        // 解析 torrent（info_hash 双口径：规范化 / 客户端原始字节）
        let torrent_id: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM torrents \
             WHERE info_hash = $1 OR raw_info_hash = $1 LIMIT 1",
        )
        .bind(&ev.hash)
        .fetch_optional(db)
        .await?;
        let Some(torrent_id) = torrent_id else {
            return Ok(true); // 种子不存在：跳过
        };
        let mut tx = db.begin().await?;
        // 幂等：同一 entry 只处理一次
        let fresh = sqlx::query(
            "INSERT INTO xreport_seen (event_id) VALUES ($1) \
             ON CONFLICT (event_id) DO NOTHING",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if fresh == 0 {
            tx.commit().await?;
            return Ok(true);
        }
        // 逐条累加佐证量；过滤自报自下载（tracker 已挡，此处双保险）
        for (uploader, bytes) in &ev.reports {
            if *bytes <= 0 || *uploader <= 0 || *uploader == ev.leecher {
                continue;
            }
            // 谎报下载对账源（0301）：按 leecher 维度落明细，
            // down_under_check 用「佐证和 vs 自报 downloaded」抓谎报下载。
            // 与佐证累加同事务，保证对账源与可信上界一致。
            sqlx::query(
                "INSERT INTO leecher_xreports \
                 (leecher, uploader, info_hash, bytes) \
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(ev.leecher)
            .bind(uploader)
            .bind(&ev.hash)
            .bind(*bytes)
            .execute(&mut *tx)
            .await?;
            // 单笔佐证量超过物理可能（>10 GiB/次 announce）→ 明显造假，
            // 记 cheat_events（带 leecher IP，供追查串通/自证）。
            if *bytes > 10 * 1024 * 1024 * 1024 {
                let _ = sqlx::query(
                    "INSERT INTO cheat_events \
                     (user_id, agent, peer_ip, reason) \
                     VALUES ($1, 'xreport:absurd', $2, \
                     'xreport_absurd（单次佐证超 10GiB，疑似串通伪造）') \
                     ON CONFLICT (user_id, agent, reason) DO UPDATE \
                       SET hits = cheat_events.hits + 1, \
                           last_seen = now()",
                )
                .bind(ev.leecher)
                .bind(&ev.ip)
                .execute(&mut *tx)
                .await;
                continue; // 该笔不入账
            }
            sqlx::query(
                "INSERT INTO upload_corroborated \
                 (uploader_id, torrent_id, bytes) \
                 VALUES ($1, $2, $3) \
                 ON CONFLICT (uploader_id, torrent_id) DO UPDATE \
                   SET bytes = upload_corroborated.bytes + EXCLUDED.bytes, \
                       updated_at = now()",
            )
            .bind(uploader)
            .bind(torrent_id)
            .bind(*bytes)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(true)
    }

    for (id, payload) in
        group::read_group(redis, &XREPORT_GROUP, &consumer).await
    {
        if handle(db, &id, &payload).await? {
            XREPORT_GROUP.ack(redis, &id).await;
            applied += 1;
        }
    }
    // 回收遗孤 pending（worker 崩溃自愈）
    for (id, payload) in
        group::reclaim_stale(redis, &XREPORT_GROUP, &consumer, 360_000).await
    {
        if handle(db, &id, &payload).await? {
            XREPORT_GROUP.ack(redis, &id).await;
            applied += 1;
        }
    }
    let _: () = {
        use redis::AsyncCommands;
        redis
            .xtrim("flux:xreport", redis::streams::StreamMaxlen::Approx(10000))
            .await
            .unwrap_or(())
    };

    // 幂等键表清理：与 announce_seen 同口径（消费循环内就地裁剪，不另开 job）
    let _ = sqlx::query(
        "DELETE FROM xreport_seen WHERE created_at < now() - interval '3 days'",
    )
    .execute(db)
    .await;

    Ok(applied)
}
