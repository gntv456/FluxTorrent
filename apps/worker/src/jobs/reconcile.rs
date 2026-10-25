//! 对账差异告警与快照对账。
//! 从 jobs.rs 按域拆出。

use sqlx::PgPool;

/// 对账告警（每 6h，先于 reconcile_snapshots 执行）：比对「流水聚合 vs 快照」，
/// 差异超阈值即 error 告警。reconcile 负责静默收敛快照漂移；本 job 负责「收敛前
/// 先留证」——付费下载不扣快照、捐赠套餐漏落流水这类「快照与流水背离」的缺陷，
/// 正是靠静默 reconcile 掩盖的（有流水注入路径绕过快照 = 真实账目差异）。
/// 注意：演示数据（0018）自带无流水的初始余额/流量，会稳定触发本告警。
/// 旧注释说「执行 0108 清理后消失」——**现网实测不成立**：0108 之后仍有 141 个
/// 「余额不为零却一条流水都没有」的账户（2026-10-19 复核）。真正让告警静默的动作
/// 是把这些种子余额登记进 `balance_baseline.base_spark`（与流量侧 `base_up/base_down`
/// 同一套机制），那是改账目的数据迁移，归站方定尺；本 job 现在负责把两类漂移
/// 分开报数，值班一眼能看出该找谁。
pub async fn reconcile_diff_alert(db: &PgPool) -> anyhow::Result<()> {
    // P-419：聚合口径必须与 `seeding.rs` 的余额重算**同一条尺**：
    //   权威余额 = balance_baseline.base_spark
    //              + sum(spark_ledger.amount 排除 kind='orphan_offset')
    // 旧公式只比 `sum(spark_ledger)` vs `sum(spark_balance)`，两处漏项：
    //   ① 没加 base_spark —— 而 maintain.rs 每月把旧分区流水**冻结进基线**
    //      （归档后主表里就没有那些行了），于是每一次冻结都让告警按冻结额
    //      永久误报；
    //   ② 没排除 orphan_offset —— 0266 挂在 root 名下的 -52374 对冲行是
    //      全站口径的系统行，个人重算不吃它（seeding.rs 注释写得很清楚），
    //      聚合比对照吃 ⇒ 平白多算 52374。
    // 现网实测（2026-10-19）：旧式 diff=-2465156，其中 52374 是 ②，
    // 余下 -2412782 全部来自 141 个「余额不为零却一条流水都没有」的演示/种子
    // 账户（0018 灌的初始余额从没进基线也没补流水）⇒ 属真实差异信号，
    // 但清账动作（把这些余额登记进 base_spark）是数据迁移，归站方定尺。
    let (ledger_spark, snapshot_spark): (i64, i64) = sqlx::query_as(
        "SELECT \
           COALESCE((SELECT sum(amount) FROM spark_ledger \
                       WHERE kind <> 'orphan_offset'), 0)::bigint \
           + COALESCE((SELECT sum(base_spark) FROM balance_baseline), \
               0)::bigint, \
           COALESCE((SELECT sum(spark_balance) FROM users), 0)::bigint",
    )
    .fetch_one(db)
    .await?;
    let spark_diff = ledger_spark - snapshot_spark;
    // 阈值 ±1000：并发窗口内的正常漂移容忍；持续超限 = 存在绕过流水的动账路径
    if spark_diff.abs() > 1000 {
        // 归因：把「谁在漂」分成两类写进日志——有流水仍漂＝真嫌疑（动账绕过了
        // 流水），无流水却有余额＝演示/种子未入基线（数据卫生）。值班读的是一句
        // 能动手的话，而不是一个要自己反查的全站差额。
        let (drift_live, drift_seed): (i64, i64) = sqlx::query_as(
            "SELECT count(*) FILTER (WHERE EXISTS \
               (SELECT 1 FROM spark_ledger l WHERE l.user_id = u.id \
                 AND l.kind <> 'orphan_offset')), \
             count(*) FILTER (WHERE NOT EXISTS \
               (SELECT 1 FROM spark_ledger l WHERE l.user_id = u.id)) \
             FROM users u WHERE u.spark_balance <> COALESCE((SELECT base_spark \
               FROM balance_baseline b WHERE b.user_id = u.id), 0) \
               + COALESCE((SELECT sum(l.amount) FROM spark_ledger l \
                 WHERE l.user_id = u.id AND l.kind <> 'orphan_offset'), 0)",
        )
        .fetch_one(db)
        .await?;
        tracing::error!(
            ledger = ledger_spark,
            snapshot = snapshot_spark,
            diff = spark_diff,
            drift_with_ledger = drift_live,
            drift_zero_ledger = drift_seed,
            "对账告警：spark 余额与「基线+流水」合计背离超阈值（drift_with_ledger \
             个账户有流水仍漂＝疑似绕过流水的动账源，drift_zero_ledger 个账户余额\
             没有任何流水＝演示/种子未登记进 balance_baseline.base_spark，属数据卫生）"
        );
    }
    let (ledger_up, snapshot_up, ledger_down, snapshot_down): (
        i64,
        i64,
        i64,
        i64,
    ) = sqlx::query_as(
        "SELECT \
           COALESCE((SELECT sum(delta_up) FROM traffic_ledger), 0)::bigint \
           + COALESCE((SELECT sum(base_up) FROM balance_baseline), \
               0)::bigint, \
           COALESCE((SELECT sum(uploaded) FROM users), 0)::bigint, \
           COALESCE((SELECT sum(delta_down) FROM traffic_ledger), 0)::bigint \
           + COALESCE((SELECT sum(base_down) FROM balance_baseline), \
               0)::bigint, \
           COALESCE((SELECT sum(downloaded) FROM users), 0)::bigint",
    )
    .fetch_one(db)
    .await?;
    // 阈值（0311 site_settings 化，anticheat_reconcile_tolerance_gb，缺省 10GiB）
    let tolerance: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings \
         WHERE name = 'anticheat_reconcile_tolerance_gb'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(10)
    .clamp(1, 1024)
        * 1024
        * 1024
        * 1024;
    // 列名只能来自下面这份字面量数组：SQL 里的标识符不能当参数绑定，
    // 数组之外没有别的取值来源。
    for (name, snap, base, delta, ledger_v, snapshot_v) in [
        (
            "uploaded",
            "uploaded",
            "base_up",
            "delta_up",
            ledger_up,
            snapshot_up,
        ),
        (
            "downloaded",
            "downloaded",
            "base_down",
            "delta_down",
            ledger_down,
            snapshot_down,
        ),
    ] {
        let diff = ledger_v - snapshot_v;
        if diff.abs() <= tolerance {
            continue;
        }
        // 归因（与 spark 侧 P-419 同一套两分法）：有流水仍漂＝真嫌疑；无流水却
        // 有快照＝演示/种子未登记进 base_*（数据卫生）。现网实测（2026-10-19）：
        // 25,165,824,000 B 的全站差额 100% 出自 4 个「一条流量流水都没有」的
        // 种子账户 ⇒ 旧文案「存在流水外的流量变动源」把数据卫生误报成缺陷，
        // 值班照它去查动账路径就是白跑一趟。
        let (drift_live, drift_seed): (i64, i64) = sqlx::query_as(&format!(
            "SELECT count(*) FILTER (WHERE EXISTS \
                   (SELECT 1 FROM traffic_ledger l WHERE l.user_id = u.id)), \
                 count(*) FILTER (WHERE NOT EXISTS \
                   (SELECT 1 FROM traffic_ledger l WHERE l.user_id = u.id)) \
                 FROM users u WHERE u.{snap} <> COALESCE((SELECT b.{base} \
                   FROM balance_baseline b WHERE b.user_id = u.id), 0) \
                   + COALESCE((SELECT sum(l.{delta}) FROM traffic_ledger l \
                     WHERE l.user_id = u.id), 0)"
        ))
        .fetch_one(db)
        .await?;
        tracing::error!(
            field = name,
            ledger = ledger_v,
            snapshot = snapshot_v,
            diff = diff,
            drift_with_ledger = drift_live,
            drift_zero_ledger = drift_seed,
            "对账告警：流量快照与「基线+流水」合计背离超阈值（drift_with_ledger \
             个账户有流水仍漂＝疑似绕过流水的流量变动源，drift_zero_ledger 个\
             账户的快照没有任何流量流水＝演示/种子未登记进 balance_baseline.\
             base_up/base_down，属数据卫生）"
        );
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
         uploaded = COALESCE((SELECT base_up FROM balance_baseline \
             WHERE user_id = users.id), 0) \
           + COALESCE((SELECT sum(delta_up) FROM traffic_ledger \
             WHERE user_id = users.id), 0), \
         downloaded = COALESCE((SELECT base_down FROM balance_baseline \
             WHERE user_id = users.id), 0) \
           + COALESCE((SELECT sum(delta_down) FROM traffic_ledger \
             WHERE user_id = users.id), 0)",
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
