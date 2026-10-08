//! 交叉上报消费（2026-10-07 P0-2 治本，自 announce_main 邻近域新建）。
//!
//! 消费 flux:xreport（tracker 校验过的「leecher 从 peer X 下载了 N 字节」），
//! 按 (uploader, torrent) 累加进 upload_corroborated —— 这是上传量的**可信上界**。
//! 计费侧（process_event）把 uploader 的 credited upload 限制在此界内，
//! 使「上传量全自报」无法做高 ratio。
//!
//! **上界本身也必须受物理约束**（2026-10-08 五轮）：佐证量同样是客户端自报的，
//! 只换了个利益方而已。不变式 = 「一个 leecher 在这颗种上能背书的总量 ≤ 它自己
//! 被记账的 credited 下载」，见 [`seeding_gate::corr_room`]。没有这条上界时
//! 这张表就是铸币机：B 每 announce 一次报 `xreport=<A>:9GiB`（单笔 < 10 GiB
//! 的 absurd 线时过去完全不设限），A 的自报上传量随即「被独立第三方确认」，
//! 而 process_event 正是看这个确认量决定还能入账多少——两个账号即可无上限
//! 做高 ratio，探针 D 组实测一条 `:1` 字节也能把幽灵做种豁免直接点亮。
//!
//! 幂等两层：
//!   ① xreport_seen（Redis 流 entry id）—— 同一 entry 重投/回收不重复累加；
//!   ② 单调累加（bytes = bytes + 本笔），即使 ① 被绕过也不会翻倍回退。

use super::group::{self, XREPORT_GROUP};
use super::process_event::seeding_gate::corr_room;
use sqlx::PgPool;

/// 单笔佐证的物理荒废线：超过它必然造假（一次 announce 之间不可能传完）。
/// 保留是因为它能**留痕**；真正的额度控制在上界账本里。
const ABSURD_BYTES: i64 = 10 * 1024 * 1024 * 1024;

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
        // 佐证者的物理额度：它自己在这颗种上被记账的下载量。
        // 用**已入账的 credited**而不是它的自报读数——否则「B 谎报下载 → B
        // 就能凭这个谎报去为 A 背书」又回到自证循环。
        let leecher_credited: i64 = sqlx::query_scalar(
            "SELECT COALESCE(downloaded, 0) FROM snatches \
             WHERE user_id = $1 AND torrent_id = $2",
        )
        .bind(ev.leecher)
        .bind(torrent_id)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);
        // 长存的上界台账（leecher_xreports 是 7 天滚动明细，不能拿它算额度）
        let already: i64 = sqlx::query_scalar(
            "SELECT credited FROM xreport_credited \
             WHERE leecher = $1 AND info_hash = $2 FOR UPDATE",
        )
        .bind(ev.leecher)
        .bind(&ev.hash)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);
        // 逐条折算可认可的佐证量；明细照记原始自报口径（对账链不动）
        let mut booked = 0i64;
        for (uploader, claimed) in &ev.reports {
            if *uploader <= 0 || *uploader == ev.leecher {
                continue;
            }
            let book = corr_room(*claimed, leecher_credited, already + booked);
            let over = (*claimed).max(0) - book;
            if over > 0 {
                // 谎报下载对账源（0301）：按 leecher 维度落明细，
                // down_under_check 用「佐证和 vs 自报 downloaded」抓谎报下载。
                // 与佐证累加同事务，保证对账源与可信上界一致。
                sqlx::query(
                    "INSERT INTO leecher_xreports \
                     (leecher, uploader, info_hash, bytes, over_bytes) \
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(ev.leecher)
                .bind(uploader)
                .bind(&ev.hash)
                .bind(*claimed)
                .bind(over)
                .execute(&mut *tx)
                .await?;
                // 单笔超物理可能（>10 GiB/次）：明显造假，留痕带 leecher IP
                if *claimed > ABSURD_BYTES {
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
                }
                // 超出自己物理额度的部分：另记一类，供管理组看「谁在超报」
                let _ = sqlx::query(
                    "INSERT INTO cheat_events \
                     (user_id, agent, peer_ip, reason) \
                     VALUES ($1, $2, $3, \
                       'xreport_over（佐证量超出本人 credited 下载，超出部分未认可）') \
                     ON CONFLICT (user_id, agent, reason) DO UPDATE \
                       SET hits = cheat_events.hits + 1, last_seen = now()",
                )
                .bind(ev.leecher)
                .bind(format!("xreport_over:{torrent_id}"))
                .bind(&ev.ip)
                .execute(&mut *tx)
                .await;
            }
            if book <= 0 {
                continue; // 额度用尽：该笔不入账
            }
            booked += book;
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
            .bind(book)
            .execute(&mut *tx)
            .await?;
        }
        if booked > 0 {
            sqlx::query(
                "INSERT INTO xreport_credited \
                 (leecher, info_hash, credited) \
                 VALUES ($1, $2, $3) \
                 ON CONFLICT (leecher, info_hash) DO UPDATE \
                   SET credited = xreport_credited.credited \
                            + EXCLUDED.credited, \
                       updated_at = now()",
            )
            .bind(ev.leecher)
            .bind(&ev.hash)
            .bind(booked)
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
