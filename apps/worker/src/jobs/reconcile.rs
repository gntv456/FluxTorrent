//! 对账差异告警与快照对账。
//! 从 jobs.rs 按域拆出。

use sqlx::PgPool;

/// 对账告警（每 6h，先于 reconcile_snapshots 执行）：比对「流水聚合 vs 快照」，
/// 差异超阈值即 error 告警。reconcile 负责静默收敛快照漂移；本 job 负责「收敛前
/// 先留证」——付费下载不扣快照、捐赠套餐漏落流水这类「快照与流水背离」的缺陷，
/// 正是靠静默 reconcile 掩盖的（有流水注入路径绕过快照 = 真实账目差异）。
/// 注意：演示数据（0018）自带无流水的初始余额/流量，会稳定触发本告警——属真实
/// 差异信号，执行 0108 清理后消失。
pub async fn reconcile_diff_alert(db: &PgPool) -> anyhow::Result<()> {
    let (ledger_spark, snapshot_spark): (i64, i64) = sqlx::query_as(
        "SELECT \
           COALESCE((SELECT sum(amount) FROM spark_ledger), 0)::bigint, \
           COALESCE((SELECT sum(spark_balance) FROM users), 0)::bigint",
    )
    .fetch_one(db)
    .await?;
    let spark_diff = ledger_spark - snapshot_spark;
    // 阈值 ±1000：并发窗口内的正常漂移容忍；持续超限 = 存在绕过流水的动账路径
    if spark_diff.abs() > 1000 {
        tracing::error!(
            ledger = ledger_spark,
            snapshot = snapshot_spark,
            diff = spark_diff,
            "对账告警：spark_ledger 合计与 users.spark_balance 合计背离超阈值（存在流水外的余额变动源，请排查）"
        );
    }
    let (ledger_up, snapshot_up, ledger_down, snapshot_down): (i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT \
               COALESCE((SELECT sum(delta_up) FROM traffic_ledger), 0)::bigint, \
               COALESCE((SELECT sum(uploaded) FROM users), 0)::bigint, \
               COALESCE((SELECT sum(delta_down) FROM traffic_ledger), 0)::bigint, \
               COALESCE((SELECT sum(downloaded) FROM users), 0)::bigint",
        )
        .fetch_one(db)
        .await?;
    // 阈值 ±10GiB
    const TRAFFIC_TOLERANCE: i64 = 10 * 1024 * 1024 * 1024;
    for (name, ledger_v, snapshot_v) in [
        ("uploaded", ledger_up, snapshot_up),
        ("downloaded", ledger_down, snapshot_down),
    ] {
        let diff = ledger_v - snapshot_v;
        if diff.abs() > TRAFFIC_TOLERANCE {
            tracing::error!(
                field = name,
                ledger = ledger_v,
                snapshot = snapshot_v,
                diff = diff,
                "对账告警：traffic_ledger 合计与 users 快照背离超阈值（存在流水外的流量变动源，请排查）"
            );
        }
    }
    let negatives: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM users WHERE spark_balance < 0 OR \
         uploaded < 0 OR downloaded < 0",
    )
    .fetch_one(db)
    .await?;
    if negatives > 0 {
        tracing::error!(
            count = negatives,
            "对账告警：{} 个用户快照为负值（超花/负流量，请核查动账路径）",
            negatives
        );
    }
    Ok(())
}

/// P0-1 兜底纠偏：全量重算 users/torrents 快照（每 6h 一次）。
/// 快照权威在流水；增量化后的点刷可能因历史漂移累积误差，低频全量对账收敛。
pub async fn reconcile_snapshots(db: &PgPool) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE users SET \
         uploaded = COALESCE((SELECT sum(delta_up) FROM traffic_ledger WHERE user_id = users.id), 0), \
         downloaded = COALESCE((SELECT sum(delta_down) FROM traffic_ledger WHERE user_id = users.id), 0)",
    )
    .execute(db)
    .await?;
    sqlx::query(
        "UPDATE torrents t SET \
         seeders = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.seeding), 0), \
         leechers = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.leeching), 0), \
         times_completed = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.completed_at IS NOT NULL), 0)",
    )
    .execute(db)
    .await?;
    Ok(())
}
