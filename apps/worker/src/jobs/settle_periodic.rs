//! 周期结算：高速标/复活/组队/愿望单/里程碑。
//! 从 jobs.rs 按域拆出。
use sqlx::{PgPool, Row};

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
    let settled = sqlx::query(
        r#"
        WITH done AS (
            UPDATE resurrections r SET status = 'done', finished_at = now()
            WHERE r.status = 'open'
              AND r.team_id IS NULL
              AND EXISTS (SELECT 1 FROM snatches s
                          WHERE s.user_id = r.user_id AND s.torrent_id = r.torrent_id
                            AND s.seeded_seconds >= r.required_hours * 3600 AND s.seeding)
            RETURNING r.id, r.user_id, r.torrent_id, r.reward_sparks
        )
        SELECT d.id, d.user_id, d.torrent_id, d.reward_sparks FROM done d
        "#,
    )
    .fetch_all(db)
    .await?;
    for row in &settled {
        let (rid, uid, tid, reward): (i64, i64, i64, i64) = (
            row.try_get(0)?,
            row.try_get(1)?,
            row.try_get(2)?,
            row.try_get(3)?,
        );
        let idem = format!("resurrection:{rid}");
        // 奖励链整段包进单事务（含幂等护栏）：CAS 已置 done 后崩溃，
        // 重启重跑此循环仍能凭幂等键补发，不再永久丢奖励。
        let mut tx = db.begin().await?;
        sqlx::query(
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
        .await?;
        sqlx::query(
            "UPDATE users SET spark_balance = spark_balance + \
         $2 WHERE id = $1 AND NOT EXISTS (SELECT 1 FROM spark_ledger WHERE \
         idempotency_key = $3)",
        )
        .bind(uid)
        .bind(reward)
        .bind(&idem)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO user_vouchers (user_id, kind, source) \
             SELECT $1, 'free', 'resurrection' \
             WHERE NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = $2)",
        )
        .bind(uid)
        .bind(&idem)
        .execute(&mut *tx)
        .await?;
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
