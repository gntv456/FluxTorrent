//! 众筹/泄漏扫描/退款结算。
//! 从 jobs.rs 按域拆出。

use sqlx::PgPool;

/// 主循环：定时任务调度。
/// 定向众筹结算（0078，HDBits Featured 口径）：
///   达标（raised ≥ goal）→ 挂 free×hours 促销 + status=1 + 通知发起人；
///   到期未达标 → 按实付全额退款（含税，税由池子承担）+ status=2。
/// 幂等：状态位先行（UPDATE ... WHERE status=0 RETURNING），重跑无副作用。
pub(crate) async fn funding_settle(db: &PgPool) -> anyhow::Result<u64> {
    // ① 达标
    let reached: Vec<(i64, i64, i32)> = sqlx::query_as(
        r#"
        UPDATE fundings SET status = 1, promoted_at = now()
        WHERE status = 0 AND raised >= goal
        RETURNING id, torrent_id, hours
        "#,
    )
    .fetch_all(db)
    .await?;
    for (fid, tid, hours) in &reached {
        sqlx::query(
            "INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source) \
             VALUES ('torrent', $1, 'free', now(), now() + make_interval(hours => $2::int), 'task') \
             ON CONFLICT DO NOTHING",
        )
        .bind(tid)
        .bind(*hours as i64)
        .execute(db)
        .await?;
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) \
             SELECT NULL, creator_id, '众筹达标', \
                    '种子 #' || $1 || ' 的众筹已达标，已挂 ' || $2 || ' 小时免费促销。' \
             FROM fundings WHERE id = $3",
        )
        .bind(tid)
        .bind(*hours as i64)
        .bind(fid)
        .execute(db)
        .await;
    }
    // ② 到期未达标 → 退款。审计修复（P1 撕裂）：旧版先置 status=2 再退款，进程在两步间
    // 死掉后该众筹永久退出结算集合（WHERE status=0 匹配不到），未完成的退款丢失。
    // 新序：CAS 到中间态 3（结算中）→ 逐笔退款 → 全部成功置 2；重启后 3 态重入续退。
    let mut expired: Vec<(i64, i64)> = sqlx::query_as(
        "UPDATE fundings SET status = 3 WHERE status = 0 AND ends_at \
         <= now() RETURNING id, creator_id",
    )
    .fetch_all(db)
    .await?;
    // 上次崩溃残留的 3 态（结算中）重新纳入本轮退款
    expired.extend(
        sqlx::query_as::<_, (i64, i64)>(
            "SELECT id, creator_id FROM fundings WHERE status = 3",
        )
        .fetch_all(db)
        .await?,
    );
    let mut refunds = 0u64;
    for (fid, _creator) in &expired {
        let contribs: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT user_id, \
             amount FROM funding_contribs WHERE funding_id = $1",
        )
        .bind(fid)
        .fetch_all(db)
        .await?;
        for (uid, amount) in &contribs {
            let idem = format!("funding-refund:{fid}:{uid}");
            // 二轮审计（P2）：旧版 INSERT 与 UPDATE 是两条 autocommit——
            // INSERT 落行后 UPDATE 的 NOT EXISTS 恒 false，退款余额永远
            // 不被这条路径加上（只落流水，靠后续 reconcile 收敛）。改为
            // 以 INSERT 的 rows_affected 判定「本次是否新入账」，新入账
            // 才无条件 UPDATE 加余额（幂等语义不变：重复执行零副作用）。
            let inserted = sqlx::query(
                r#"
                INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
                SELECT nextval('spark_ledger_id_seq'), $1, $2, 'funding_refund', $3
                WHERE NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = $3)
                "#,
            )
            .bind(uid)
            .bind(amount)
            .bind(&idem)
            .execute(db)
            .await?
            .rows_affected();
            if inserted > 0 {
                sqlx::query(
                    "UPDATE users SET spark_balance = spark_balance + $2                      WHERE id = $1",
                )
                .bind(uid)
                .bind(amount)
                .execute(db)
                .await?;
                refunds += 1;
            }
        }
        // 全部退款成功 → 终态 2（失败时下一轮从 3 态重入续退，幂等键防双退）
        sqlx::query("UPDATE fundings SET status = 2 WHERE id = $1")
            .bind(fid)
            .execute(db)
            .await?;
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) \
             SELECT NULL, creator_id, '众筹未达标', \
                    '种子相关众筹到期未达标，参与者的魔力已全额退款（含赠送税部分）。' \
             FROM fundings WHERE id = $1",
        )
        .bind(fid)
        .execute(db)
        .await;
    }
    if !reached.is_empty() || !expired.is_empty() {
        tracing::info!(
            reached = reached.len(),
            expired = expired.len(),
            refunds,
            "funding_settle"
        );
    }
    Ok((reached.len() + expired.len()) as u64)
}

/// 泄露者检测（0078，U3D LeakerController 离线简化版）：
///   ① passkey 多地并发：7 天内同一用户下载行为来自 ≥3 个不同公网 IP 且跨运营商
///      级网段（/16 不同）→ 疑似 passkey 泄露；
///   ② 首发抢发：种子过审后 10 分钟内即出现完成下载的非常规 agent。
/// 数据源 login_events（已有 IP 记录）+ traffic_ledger。只写 leak_events 供 staff 复核，不自动处罚。
pub(crate) async fn leak_scan(db: &PgPool) -> anyhow::Result<u64> {
    // ① passkey 多段 IP（登录侧证据）：7 天内成功登录跨 ≥3 个 /16（v6 为 /32）网段
    let susp1: Vec<(i64, i16, serde_json::Value)> = sqlx::query_as(
        r#"
        WITH grouped AS (
          SELECT user_id,
                 count(DISTINCT CASE WHEN family(ip) = 4 THEN set_masklen(ip, 16) END) AS v4_segs,
                 count(DISTINCT CASE WHEN family(ip) = 6 THEN set_masklen(ip, 32) END) AS v6_segs,
                 count(DISTINCT ip) AS ips
          FROM login_events
          WHERE ok AND ip IS NOT NULL AND created_at > now() - interval '7 days'
          GROUP BY user_id
        )
        SELECT user_id,
               LEAST(100, 40 + 15 * GREATEST(v4_segs, v6_segs))::smallint AS score,
               jsonb_build_object('ips', ips, 'v4_segs', v4_segs, 'v6_segs', v6_segs) AS detail
        FROM grouped
        WHERE GREATEST(v4_segs, v6_segs) >= 3
          AND NOT EXISTS (SELECT 1 FROM users u WHERE u.id = grouped.user_id AND u.class_id >= 90)
        "#,
    )
    .fetch_all(db)
    .await?;
    // ② 首发快速完成（下载侧证据）：过审 10 分钟内即完成下载（抢发/泄露的典型特征）
    let susp2: Vec<(i64, i16, serde_json::Value)> = sqlx::query_as(
        r#"
        SELECT s.user_id, 60::smallint,
               jsonb_build_object('torrent_id', s.torrent_id,
                                  'completed_at', s.completed_at,
                                  'approved_at', t.approved_at,
                                  'downloaded', s.downloaded)
        FROM snatches s JOIN torrents t ON t.id = s.torrent_id
        WHERE s.completed_at IS NOT NULL
          AND t.approved_at IS NOT NULL
          AND s.completed_at < t.approved_at + interval '10 minutes'
          AND t.approved_at > now() - interval '7 days'
          AND NOT EXISTS (SELECT 1 FROM users u WHERE u.id = s.user_id AND u.class_id >= 90)
        "#,
    )
    .fetch_all(db)
    .await?;
    let mut n = 0u64;
    // 两类来源分开写（kind 语义清晰），去重键 (user, kind, torrent)×3 天窗
    for (uid, score, detail) in susp1.iter() {
        let r = sqlx::query(
            r#"
            INSERT INTO leak_events (kind, user_id, torrent_id, detail, score)
            SELECT 'passkey_multi_ip', $1, NULL, $2, $3
            WHERE NOT EXISTS (
                SELECT 1 FROM leak_events e
                WHERE e.user_id = $1 AND e.kind = 'passkey_multi_ip'
                  AND e.created_at > now() - interval '3 days'
            )
            "#,
        )
        .bind(uid)
        .bind(detail)
        .bind(*score)
        .execute(db)
        .await?;
        n += r.rows_affected();
    }
    for (uid, score, detail) in susp2.iter() {
        let tid = detail.get("torrent_id").and_then(|v| v.as_i64());
        let r = sqlx::query(
            r#"
            INSERT INTO leak_events (kind, user_id, torrent_id, detail, score)
            SELECT 'first_dl_fast', $1, $2, $3, $4
            WHERE NOT EXISTS (
                SELECT 1 FROM leak_events e
                WHERE e.user_id = $1 AND e.kind = 'first_dl_fast'
                  AND (e.torrent_id IS NOT DISTINCT FROM $2)
                  AND e.created_at > now() - interval '3 days'
            )
            "#,
        )
        .bind(uid)
        .bind(tid)
        .bind(detail)
        .bind(*score)
        .execute(db)
        .await?;
        n += r.rows_affected();
    }
    if n > 0 {
        tracing::warn!(n, "leak_scan: 可疑事件写入 leak_events，待 staff 复核");
    }
    Ok(n)
}

/// Refundable 结算（0078，U3D 口径：边做种边退下载量）：
/// 促销档 kind='refundable' 生效期间，参与者下载量按「做种时长线性退还」——
/// 每 24h 做种退该种计费下载的 1/14（两周退满，与 H&R 120h 宽限同量级）。
/// 退款以负流量流水落 traffic_ledger（delta_down 为负），users 快照由 reconcile 聚合。
/// 幂等：当日已退过（同用户同种当日存在负流水）不再退——一天一结。
pub(crate) async fn refundable_settle(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        WITH eligible AS (
            SELECT s.user_id, s.torrent_id, s.downloaded
            FROM snatches s
            JOIN promotions p ON p.torrent_id = s.torrent_id
              AND p.kind = 'refundable' AND p.starts_at <= now() AND p.ends_at > now()
            WHERE s.downloaded > 0 AND s.seeded_seconds >= 86400
        ),
        due AS (
            SELECT user_id, torrent_id, downloaded,
                   -LEAST(downloaded, downloaded / 14) AS refund
            FROM eligible
        )
        INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, delta_down, window_start)
        SELECT nextval('traffic_ledger_id_seq'), user_id, torrent_id, 0, refund, now()
        FROM due d
        WHERE NOT EXISTS (
            SELECT 1 FROM traffic_ledger l
            WHERE l.user_id = d.user_id AND l.torrent_id = d.torrent_id
              AND l.delta_down < 0 AND l.window_start > current_date
        )
        "#,
    )
    .execute(db)
    .await?;
    if res.rows_affected() > 0 {
        tracing::info!(
            n = res.rows_affected(),
            "refundable_settle: 下载量退还落账"
        );
    }
    Ok(res.rows_affected())
}
