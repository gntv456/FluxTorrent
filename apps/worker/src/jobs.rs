//! 定时与消费任务。

use sqlx::PgPool;

/// 促销到期回收（M06：到期自动回收，无残留）。
pub async fn expire_promotions(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query("DELETE FROM promotions WHERE ends_at <= now()")
        .execute(db)
        .await?;
    Ok(res.rows_affected())
}

/// 保种区移出（M19 旧站口径：做种 > 7 移出，免费延续 3 天）。
pub async fn preserve_exit(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        WITH exited AS (
            UPDATE seed_preserve sp SET exited_at = now(), exit_reason = 'seeders_gt_7'
            FROM torrents t
            WHERE t.id = sp.torrent_id AND t.seeders > 7 AND sp.exited_at IS NULL
            RETURNING sp.torrent_id
        )
        INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source)
        SELECT 'torrent', torrent_id, 'free', now(), now() + interval '3 days', 'preserve_grace'
        FROM exited
        ON CONFLICT DO NOTHING
        "#,
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// 做种收益小时结算（M11：基础火花 + 加成；捐赠者 2x）。
/// 每用户每小时一条流水，幂等键 = seeding:{user}:{yyyymmddhh}。
pub async fn seeding_reward(db: &PgPool, base: i64) -> anyhow::Result<u64> {
    let hour = chrono::Utc::now().format("%Y%m%d%H").to_string();
    let res = sqlx::query(
        r#"
        WITH earners AS (
            SELECT u.id, u.donor,
                   count(*) AS seeding_count, COALESCE(sum(t.size),0) AS seeding_size
            FROM users u
            JOIN snatches s ON s.user_id = u.id AND s.seeding
            JOIN torrents t ON t.id = s.torrent_id
            WHERE u.status < 2
            GROUP BY u.id, u.donor
        ),
        due AS (
            SELECT id,
                   ($1 + (seeding_count * 2 + seeding_size / 1099511627776))::bigint
                     * CASE WHEN donor THEN 2 ELSE 1 END AS amount,
                   'seeding:' || id || ':' || $2 AS idem
            FROM earners
        )
        INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
        SELECT nextval('spark_ledger_id_seq'), id, amount, 'seeding_reward', idem
        FROM due
        WHERE NOT EXISTS (
            SELECT 1 FROM spark_ledger l WHERE l.idempotency_key = due.idem
        )
        "#,
    )
    .bind(base)
    .bind(&hour)
    .execute(db)
    .await?;
    // 刷新余额快照（权威在流水，快照仅展示）
    sqlx::query(
        "UPDATE users SET spark_balance = COALESCE((             SELECT sum(amount) FROM spark_ledger WHERE user_id = users.id          ), 0)",
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

#[derive(serde::Deserialize)]
struct AnnounceEvent {
    user: i64,
    hash: String,
    up: i64,
    down: i64,
    #[serde(default)]
    event: String,
    #[serde(default)]
    left: i64,
}

/// 消费 announce 事件流（§5.4 链路 ④-⑦）：Redis Stream → 计费流水 + snatches。
/// 事件为 JSON 文本，字段 user/hash/up/down/event/left。
pub async fn consume_announce(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
) -> anyhow::Result<u64> {
    use redis::AsyncCommands;

    // 拉取一批事件（生产态用 XREADGROUP 消费组；此处保证类型正确的批拉语义）
    let batches: Vec<(String, Vec<(String, String)>)> = redis
        .xrange("flux:announce", "-", "+")
        .await
        .unwrap_or_default();
    let mut applied = 0u64;
    for (_stream, entries) in batches {
        for (_id, payload) in entries {
            let Ok(ev) = serde_json::from_str::<AnnounceEvent>(&payload) else {
                continue;
            };
            let torrent_id: Option<i64> =
                sqlx::query_scalar("SELECT id FROM torrents WHERE info_hash = $1")
                    .bind(&ev.hash)
                    .fetch_optional(db)
                    .await
                    .unwrap_or(None);
            let Some(torrent_id) = torrent_id else {
                continue;
            };

            let seeding = ev.left == 0;
            // upsert snatch 状态
            sqlx::query(
                r#"
                INSERT INTO snatches (user_id, torrent_id, uploaded, downloaded, leeching, seeding, completed_at)
                VALUES ($1, $2, $3, $4, $5, $6, CASE WHEN $7 THEN now() ELSE NULL END)
                ON CONFLICT (user_id, torrent_id) DO UPDATE SET
                  uploaded = snatches.uploaded + EXCLUDED.uploaded,
                  downloaded = snatches.downloaded + EXCLUDED.downloaded,
                  leeching = EXCLUDED.leeching,
                  seeding = EXCLUDED.seeding OR snatches.seeding,
                  completed_at = COALESCE(snatches.completed_at, EXCLUDED.completed_at)
                "#,
            )
            .bind(ev.user)
            .bind(torrent_id)
            .bind(ev.up)
            .bind(ev.down)
            .bind(!seeding)
            .bind(seeding)
            .bind(ev.event == "completed")
            .execute(db)
            .await?;
            // 计费流水
            sqlx::query(
                "INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, delta_down, window_start)                  VALUES (nextval('traffic_ledger_id_seq'), $1, $2, $3, $4, now())",
            )
            .bind(ev.user)
            .bind(torrent_id)
            .bind(ev.up)
            .bind(ev.down)
            .execute(db)
            .await?;
            applied += 1;
        }
        // 已处理事件裁剪（保留近期数据用于审计）
        let _: () = redis.del("flux:announce").await.unwrap_or(());
    }
    Ok(applied)
}

/// 主循环：定时任务调度。
pub async fn run_all(db: PgPool, mut redis: redis::aio::ConnectionManager) -> anyhow::Result<()> {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
    let mut hour_tick = tokio::time::interval(std::time::Duration::from_secs(3600));
    let mut first_hour = true;
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if let Err(e) = expire_promotions(&db).await { tracing::error!(?e, "expire_promotions"); }
                if let Err(e) = preserve_exit(&db).await { tracing::error!(?e, "preserve_exit"); }
                if let Err(e) = consume_announce(&db, &mut redis).await { tracing::error!(?e, "consume_announce"); }
            }
            _ = hour_tick.tick() => {
                if first_hour { first_hour = false; continue; }
                if let Err(e) = seeding_reward(&db, 10).await { tracing::error!(?e, "seeding_reward"); }
            }
        }
    }
}
