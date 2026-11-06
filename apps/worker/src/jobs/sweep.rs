//! 清扫类：过期 peer/登录事件/邀请/token/DLQ。
//! 从 jobs.rs 按域拆出。

use super::staff_alert::staff_alert;
use sqlx::PgPool;
use std::collections::BTreeSet;

/// 单批清理行数。批间独立事务 ⇒ 行锁持有时间与批大小同量级。
const SWEEP_BATCH_ROWS: i64 = 5_000;
/// 单轮清理的最大批数（5,000 × 200 = 100 万行上限，防失控循环）。
const SWEEP_MAX_BATCHES: u32 = 200;

/// 僵尸 peer 判定阈值（秒）= max(2h, 2×announce_interval)。
/// 同时用于两处：① `sweep_stale_peers` 清理标记；② `seeding_reward` 结算前的数据新鲜度过滤
/// （结算是发钱动作，不能依赖"另一个 job 恰好跑过"）。
pub(crate) async fn stale_peer_threshold_secs(db: &PgPool) -> i64 {
    let interval: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'announce_interval'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .map(|v| v.clamp(60, 86400))
    .unwrap_or(1800);
    (interval * 2).max(7200)
}

/// 僵尸做种/下载标记清理：tracker peer 表 90s 超时即除名，但 DB 侧 snatches.seeding/leeching
/// 原本只在下一次 announce 时被覆盖 —— 客户端崩溃/卸载（无 stopped 事件）的行会永久保持
/// seeding=true，导致 seeding_reward 空转发钱（live 证据：27 行 last_seen 2 天前仍在领收益）
/// 与 torrents.seeders 虚高。阈值 = max(2h, 2×announce_interval)，远大于正常重汇报抖动。
///
/// 同一事务内把受影响 torrent 的三计数从 snatches 重算：只清标记会让
/// torrents.seeders/leechers 虚高到下一次 6h 全量对账（MZ8 漂移）。
/// 必须分两条语句——同一条语句里的数据修改 CTE 彼此看不到对方改后的行，
/// 计数会按「清扫前」的快照算出来（实测仍残留 1）。
///
/// ⚠️ 锁纪律（2026-11-06 线上事故）：本 job 必须**分批 + 批间提交**。
/// 迁站导入的历史行全带 seeding=true 且 last_seen_at 是旧站时间戳，首轮
/// 要清 8.5 万行 / 2.2 万个种子。旧写法（一个事务清完全部 + 逐行相关
/// 子查询重算三计数）实测持锁 90 秒以上：
///   ① `UPDATE snatches` 锁住 8.5 万行 → announce 的
///      `SELECT ... FOR UPDATE`（ledger_guard 取基线）排在后面，30+
///      会话同时等锁 → worker 连接池（DB_POOL_SIZE=32）被占满 → 新事件
///      报 "pool timed out while waiting for an open connection"；
///   ② 同事务里的 `UPDATE torrents` 持 FOR NO KEY UPDATE 锁，与 announce
///      写 snatches 时 FK（torrent_id → torrents.id）要的 FOR KEY SHARE
///      直接冲突。
/// 现在改为：分批清标记（每批 5k 行、批间独立短事务）→ 一次聚合重算受
/// 影响种子的 seeders/leechers（0464 部分索引支撑，扫描量与被清掉的行
/// 同量级），点刷事务另设 lock_timeout，锁争用时给 announce 让路。
///
/// 点刷只重算 seeders/leechers：times_completed 取决于 completed_at，
/// 清标记不改变它，重算纯属多扫一遍 70 万行。
pub(crate) async fn sweep_stale_peers(db: &PgPool) -> anyhow::Result<u64> {
    let threshold_secs = stale_peer_threshold_secs(db).await;
    let mut swept_total: u64 = 0;
    let mut affected: BTreeSet<i64> = BTreeSet::new();
    // keyset 游标：按 last_seen_at 升序推进（partial index
    // idx_snatches_last_seen_active 天然按该列有序，LIMIT 可提前收口）。
    // 已清标记的行不再满足 (seeding OR leeching) → 自动退出候选集，
    // 不会出现「同一批反复重放」的死循环。
    let mut cursor: Option<chrono::DateTime<chrono::Utc>> = None;
    for _ in 0..SWEEP_MAX_BATCHES {
        let batch: Vec<(i64, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
            // raw string：换行与缩进原样保留。Rust 的 `\` 续行会吃掉下一行
            // 的前导空白，SQL 里 `t\` + `SET …` 会拼成 `tSET …`（见
            // peer_counts.rs 头注②），这类长 SQL 一律不用续行拼接。
            r#"
            WITH victims AS (
                SELECT s.user_id, s.torrent_id, s.last_seen_at
                  FROM snatches s
                 WHERE (s.seeding OR s.leeching)
                   AND s.last_seen_at < now() -
                       ($1::bigint * interval '1 second')
                   AND s.last_seen_at >=
                       coalesce($2::timestamptz, '-infinity'::timestamptz)
                 ORDER BY s.last_seen_at
                 LIMIT $3
            )
            UPDATE snatches s
               SET seeding = FALSE, leeching = FALSE
              FROM victims v
             WHERE s.user_id = v.user_id AND s.torrent_id = v.torrent_id
            RETURNING s.torrent_id, s.last_seen_at
            "#,
        )
        .bind(threshold_secs)
        .bind(cursor)
        .bind(SWEEP_BATCH_ROWS)
        .fetch_all(db)
        .await?;
        if batch.is_empty() {
            break;
        }
        // RETURNING 的顺序不保证与扫描序一致，游标取本批最大值。
        if let Some(m) = batch.iter().map(|(_, s)| *s).max() {
            cursor = Some(m);
        }
        for (tid, _) in &batch {
            affected.insert(*tid);
        }
        swept_total += batch.len() as u64;
    }
    if !affected.is_empty() {
        refresh_peer_counts(db, &affected).await?;
    }
    Ok(swept_total)
}

/// 点刷受影响种子的 seeders/leechers：一次聚合 + 哈希连接，替代逐行
/// 相关子查询（后者对 2.2 万个种子做 3 次子查询 = 6.6 万次索引探查）。
/// 独立事务 + lock_timeout：与 announce 争 FK 锁时主动让路，不把整条
/// 计费链路拖停。分批与归零写回见 peer_counts.rs 头注。
async fn refresh_peer_counts(
    db: &PgPool,
    affected: &BTreeSet<i64>,
) -> anyhow::Result<()> {
    let ids: Vec<i64> = affected.iter().copied().collect();
    let mut tx = db.begin().await?;
    sqlx::query("SET LOCAL lock_timeout = '5s'")
        .execute(&mut *tx)
        .await?;
    super::peer_counts::refresh_peer_counts_in_tx(&mut tx, &ids).await?;
    tx.commit().await?;
    Ok(())
}

/// 登录事件留存清理：IP 属个人信息，90 天后删除（每小时一次，幂等）。
pub(crate) async fn purge_old_login_events(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        "DELETE FROM login_events WHERE created_at < \
     now() - interval '90 days'",
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// 过期邀请落库回收（NP docleanup 口径）：status=0 且过期的邀请码统一置 status=2。
/// 此前仅展示层 CASE 折算，库内 status 恒 0——按 status 统计的后台口径失真。
pub(crate) async fn expire_invites(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        "UPDATE invites SET status = 2 WHERE status = 0 \
     AND expires_at <= now()",
    )
    .execute(db)
    .await?;
    if res.rows_affected() > 0 {
        tracing::info!(n = res.rows_affected(), "expired invites recycled");
    }
    Ok(res.rows_affected())
}

/// 一次性凭证清理：download_keys（30 分钟）与 password_resets（30 分钟）过期即删。
/// password_resets 原本只在手动 POST /admin/docleanup 里清，无人点击则永久堆积。
/// 过期勋章下线（0287 P2）：wearing 置 false 保留行。
/// 用户侧读取全带 expires_at > now() 过滤（表现正确），但 admin 持有面板
/// 不过滤会把过期勋章当有效显示；佩戴标志不清理也会让「过期仍佩戴」
/// 的中间态残留。不 DELETE——发放审计需要行存在以追溯。
pub(crate) async fn expire_medals(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        "UPDATE user_medals SET wearing = false \
         WHERE wearing AND expires_at IS NOT NULL \
           AND expires_at <= now()",
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

pub(crate) async fn purge_expired_tokens(db: &PgPool) -> anyhow::Result<u64> {
    let a = sqlx::query("DELETE FROM download_keys WHERE expires_at < now()")
        .execute(db)
        .await?
        .rows_affected();
    let b = sqlx::query("DELETE FROM password_resets WHERE expires_at < now()")
        .execute(db)
        .await?
        .rows_affected();
    Ok(a + b)
}

/// 死信队列可见性（卫生 P1）：DLQ 只进不出等于变相丢计费。
/// 有积压时通知管理组信箱（复用 cheat_audit 告警模式），同一批积压只告警一次。
/// 0227 审查 P2：agent_block 死信分键（flux:agentblock:dlq），与计费死信
/// 分开统计、文案分源——两队列共用告警节奏。
pub(crate) async fn dlq_watch(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
) -> anyhow::Result<u64> {
    use redis::AsyncCommands;
    let a_len: i64 = redis.llen("flux:announce:dlq").await.unwrap_or(0);
    let b_len: i64 = redis.llen("flux:agentblock:dlq").await.unwrap_or(0);
    let x_len: i64 = redis.llen("flux:xreport:dlq").await.unwrap_or(0);
    let len: i64 = a_len + b_len + x_len;
    if len == 0 {
        // 队列清空后复位告警游标，下批积压可再次告警
        let _: () = redis.del("flux:announce:dlq:alerted").await.unwrap_or(());
        return Ok(0);
    }
    let alerted: i64 =
        redis.get("flux:announce:dlq:alerted").await.unwrap_or(0);
    if alerted == 0 {
        let body = format!(
            "死信队列当前积压 {len} 条（连续失败 6 次进入）：计费流 \
             flux:announce:dlq {a_len} 条（计费已跳过需人工补偿）、客户端拦截流 \
             flux:agentblock:dlq {b_len} 条（仅告警记录）、交叉佐证流 \
             flux:xreport:dlq {x_len} 条（佐证未入账需人工补算）。请按队列排查。"
        );
        staff_alert(db, "announce 死信队列积压告警", &body, "cheater").await;
        let _: () = redis
            .set_ex("flux:announce:dlq:alerted", 1, 24 * 3600)
            .await
            .unwrap_or(());
        tracing::warn!(len, "announce DLQ backlog alerted");
    }
    Ok(len as u64)
}
