//! 周期结算：高速标/复活/组队/愿望单/里程碑。
//! 从 jobs.rs 按域拆出。
use sqlx::PgPool;

/// 盒子/高速做种打标（0077，U3D AutoHighspeedTag 口径）：近 7 天 snatches 上传统计速度
/// 超 100MB/s 视为高速线路在做种 → torrents.highspeed = true（展示与激励用，不惩罚）。
pub(crate) async fn highspeed_tag(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        WITH speeds AS (
            -- 平均上速 = 累计上传 / max(做种秒数, 1h)：对短时突发鲁棒（U3D 用墙钟，我们做种口径更准）
            SELECT torrent_id, max(uploaded / GREATEST(seeded_seconds::bigint, 3600)) AS bps
            FROM snatches WHERE uploaded > 0 AND last_seen_at > now() - interval '7 days'
            GROUP BY torrent_id
        )
        UPDATE torrents t SET highspeed = TRUE
        FROM speeds s WHERE t.id = s.torrent_id AND s.bps > 104857600 AND NOT t.highspeed
        "#,
    )
    .execute(db)
    .await?;
    if res.rows_affected() > 0 {
        tracing::info!(n = res.rows_affected(), "highspeed torrents tagged");
    }
    Ok(res.rows_affected())
}

/// 复活任务自动验收（0073，U3D Graveyard 口径）：领取者补种累计时长 ≥ required_hours
/// 且当前仍在做种 → 发奖（火花 + 1 枚免费券）+ 种子挂 7 天 free bump + 站内信。
/// 幂等：状态 CAS（open→done），奖励只随成功转移发放一次。
pub(crate) async fn resurrection_settle(db: &PgPool) -> anyhow::Result<u64> {
    // 三轮审计 P2（2026-10-07）：候选集只读（不再 CTE 内置 done）——旧版
    // 先 autocommit 置 done 再循环发奖，中间崩溃的行被重跑的
    // status='open' 排除，奖励永久丢失。置位挪进每行发奖事务（见下）。
    let settled = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        r#"
        SELECT r.id, r.user_id, r.torrent_id, r.reward_sparks
        FROM resurrections r
        WHERE r.status = 'open'
          AND r.team_id IS NULL
          AND EXISTS (SELECT 1 FROM snatches s
                      WHERE s.user_id = r.user_id
                        AND s.torrent_id = r.torrent_id
                        AND s.seeded_seconds >= r.required_hours * 3600
                        AND s.seeding
                        -- P0-2（2026-10-07 保种组审计）：幽灵挂种不计——实测
                        -- port=0 纯 curl announce 无文件无下载即可累计做种时长
                        -- 领取复活奖励；补与做种收益同口径的端口/回连过滤，
                        -- 另要求存在真实下载（completed 侧证据：累计下载>0）。
                        AND s.last_port > 0
                        AND NOT COALESCE(s.connectable = 0, false)
                        AND s.downloaded > 0)
        "#,
    )
    .fetch_all(db)
    .await?;
    for &(rid, uid, tid, reward) in &settled {
        let idem = format!("resurrection:{rid}");
        // 状态置位（open→done）与发奖同事务：回滚时状态回滚，重跑完整重试
        let mut tx = db.begin().await?;
        let claimed = sqlx::query(
            "UPDATE resurrections SET status = 'done', finished_at = now()              WHERE id = $1 AND status = 'open'",
        )
        .bind(rid)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if claimed == 0 {
            tx.rollback().await?;
            continue; // 并发 worker 已处理
        }
        // 奖励链与上方状态置位共用同一事务（幂等护栏只在 INSERT 上判断：
        // 事务内已插入的行对本事务可见，UPDATE/赠券再带同款 NOT EXISTS
        // 会恒不命中 → 整体回滚 → 奖励静默丢失——链路缺陷 #10 同款教训）。
        let inserted = sqlx::query(
            r#"
            INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
            SELECT nextval('spark_ledger_id_seq'), $1, $2, 'resurrection', $3
            WHERE NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = $3)
            "#,
        )
        .bind(uid)
        .bind(reward)
        .bind(&idem)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            > 0;
        if inserted {
            sqlx::query(
                "UPDATE users SET spark_balance = spark_balance + $2 \
                 WHERE id = $1",
            )
            .bind(uid)
            .bind(reward)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "INSERT INTO user_vouchers (user_id, kind, source) \
                 VALUES ($1, 'free', 'resurrection')",
            )
            .bind(uid)
            .execute(&mut *tx)
            .await?;
        }
        // 7 天 free bump（U3D 口径）：全站看见的即时激励
        sqlx::query(
            "INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source) \
             VALUES ('torrent', $1, 'free', now(), now() + interval '7 days', 'task') \
             ON CONFLICT DO NOTHING",
        )
        .bind(tid)
        .execute(&mut *tx)
        .await?;
        let _ = sqlx::query(
                        "INSERT INTO messages (sender_id, receiver_id, \
             subject, body) VALUES (NULL, $1, $2, $3)",
        )
        .bind(uid)
        .bind("复活任务完成")
        .bind(format!(
            "恭喜！你复活种子 #{tid} 的任务已验收：奖励 {reward} 魔力 + 1 枚免费券，\
             该种子已获得 7 天免费促销。感谢你为保种做出的贡献！"
        ))
        .execute(&mut *tx)
        .await;
        tx.commit().await?;
    }
    if !settled.is_empty() {
        tracing::info!(n = settled.len(), "resurrections settled");
    }
    Ok(settled.len() as u64)
}
