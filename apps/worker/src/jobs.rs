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
    let redis_dead_letter = redis.clone();
    use redis::AsyncCommands;

    // 游标消费：从上次处理到的 ID 继续拉取（Redis 键持久化游标，重启不丢事件、不重复计费）
    let last_id: Option<String> = redis.get("flux:announce:cursor").await.unwrap_or(None);
    let from = last_id.clone().unwrap_or_else(|| "-".to_string());

    // XRANGE → StreamRangeReply（redis 0.27 类型映射；错误必须可见，不允许静默空消费）
    let reply = match redis
        .xrange::<_, _, _, redis::streams::StreamRangeReply>("flux:announce", &from, "+")
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(?e, "xrange flux:announce 失败");
            return Err(e.into());
        }
    };

    let mut applied = 0u64;
    let mut last_seen_id: Option<String> = None;
    for entry in reply.ids {
        let id = entry.id;
        // 已处理过的游标本条跳过
        if Some(&id) == last_id.as_ref() {
            last_seen_id = Some(id);
            continue;
        }
        let Some(payload) = entry
            .map
            .get("payload")
            .and_then(|v| redis::from_redis_value::<String>(v).ok())
        else {
            // 损坏事件进死信，不阻塞游标
            tracing::warn!(%id, "announce 事件损坏，进死信");
            let mut conn_dl = redis_dead_letter.clone();
            let _: Result<(), _> = redis::cmd("RPUSH")
                .arg("flux:announce:dlq")
                .arg(&id)
                .query_async(&mut conn_dl)
                .await;
            last_seen_id = Some(id);
            continue;
        };
        let ev = match serde_json::from_str::<AnnounceEvent>(&payload) {
            Ok(ev) => ev,
            Err(e) => {
                tracing::warn!(%id, ?e, "事件 JSON 解析失败，进死信");
                let mut conn_dl = redis_dead_letter.clone();
                let _: Result<(), _> = redis::cmd("RPUSH")
                    .arg("flux:announce:dlq")
                    .arg(&payload)
                    .query_async(&mut conn_dl)
                    .await;
                last_seen_id = Some(id);
                continue;
            }
        };
        match process_event(db, &ev).await {
            Ok(()) => {
                applied += 1;
                last_seen_id = Some(id);
            }
            Err(e) => {
                // 处理失败：不推进游标，下轮重试（避免丢失计费）
                tracing::error!(%id, ?e, "事件计费失败，游标暂停等待重试");
                break;
            }
        }
    }
    if let Some(id) = last_seen_id {
        let cur: Result<(), redis::RedisError> = redis.set("flux:announce:cursor", &id).await;
        if let Err(e) = cur {
            // 游标写入失败必须显式报错：静默失败会导致下轮重复计费
            tracing::error!(?e, "游标写入失败（下轮可能重复计费，需人工核对）");
            return Err(e.into());
        }
        let _: () = redis
            .xtrim("flux:announce", redis::streams::StreamMaxlen::Approx(10000))
            .await
            .unwrap_or(());
    }
    // 刷新用户上/下载量快照（权威在 traffic_ledger 流水，§6.2 快照仅展示）
    sqlx::query(
        "UPDATE users SET             uploaded = COALESCE((SELECT sum(delta_up) FROM traffic_ledger WHERE user_id = users.id), 0),             downloaded = COALESCE((SELECT sum(delta_down) FROM traffic_ledger WHERE user_id = users.id), 0)",
    )
    .execute(db)
    .await?;
    // 回填种子做种/下载计数（详情页与保种规则数据源）
    sqlx::query(
        "UPDATE torrents t SET             seeders = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.seeding), 0),             leechers = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.leeching), 0),             times_completed = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.completed_at IS NOT NULL), 0)",
    )
    .execute(db)
    .await?;
    Ok(applied)
}

/// 单事件计费（促销裁决 + snatch upsert + 流水）
async fn process_event(db: &PgPool, ev: &AnnounceEvent) -> anyhow::Result<()> {
    // 未知种子的查询失败必须显式报错（重试），不能静默丢弃计费
    let torrent_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM torrents WHERE info_hash = $1")
            .bind(&ev.hash)
            .fetch_optional(db)
            .await?;
    let Some(torrent_id) = torrent_id else {
        return Ok(()); // 种子确实不存在：跳过
    };

    // 促销快照裁决（§5.4-⑦）
    let kind: Option<String> = sqlx::query_scalar(
        "SELECT kind::text FROM promotions WHERE torrent_id = $1 AND starts_at <= now() AND ends_at > now() ORDER BY id DESC LIMIT 1",
    )
    .bind(torrent_id)
    .fetch_optional(db)
    .await?;
    let global: Option<String> = sqlx::query_scalar(
        "SELECT kind::text FROM promotions WHERE scope = 'global' AND starts_at <= now() AND ends_at > now() ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(db)
    .await?;
    let (up_mult, down_mult) = billing_multipliers(kind.as_deref(), global.as_deref());

    // BEP3：ev.up/down 是客户端累计总量 —— 先取出上次上报值换算增量（P0 修复）
    let mut tx = db.begin().await?;
    let last: Option<(i64, i64)> = sqlx::query_as(
        "SELECT last_up, last_down FROM snatches WHERE user_id = $1 AND torrent_id = $2 FOR UPDATE",
    )
    .bind(ev.user)
    .bind(torrent_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (last_up, last_down) = last.unwrap_or((0, 0));
    // 计数器回绕/客户端重置时按 0 处理
    let raw_up = (ev.up - last_up).max(0);
    let raw_down = (ev.down - last_down).max(0);
    let delta_up = (raw_up as f64 * up_mult) as i64;
    let delta_down = (raw_down as f64 * down_mult) as i64;

    let seeding = ev.left == 0;
    sqlx::query(
        r#"
        INSERT INTO snatches (user_id, torrent_id, uploaded, downloaded, last_up, last_down, leeching, seeding, completed_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, CASE WHEN $9 THEN now() ELSE NULL END)
        ON CONFLICT (user_id, torrent_id) DO UPDATE SET
          uploaded = snatches.uploaded + EXCLUDED.uploaded,
          downloaded = snatches.downloaded + EXCLUDED.downloaded,
          last_up = EXCLUDED.last_up,
          last_down = EXCLUDED.last_down,
          leeching = EXCLUDED.leeching,
          seeding = EXCLUDED.seeding OR snatches.seeding,
          completed_at = COALESCE(snatches.completed_at, EXCLUDED.completed_at)
        "#,
    )
    .bind(ev.user)
    .bind(torrent_id)
    .bind(raw_up)
    .bind(raw_down)
    .bind(ev.up)
    .bind(ev.down)
    .bind(!seeding)
    .bind(seeding)
    .bind(ev.event == "completed")
    .execute(&mut *tx)
    .await?;
    // 仅在有实际增量时落流水（避免零增量噪声）
    if delta_up > 0 || delta_down > 0 {
        sqlx::query(
            "INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, delta_down, window_start)          VALUES (nextval('traffic_ledger_id_seq'), $1, $2, $3, $4, now())",
        )
        .bind(ev.user)
        .bind(torrent_id)
        .bind(delta_up)
        .bind(delta_down)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// 做种里程碑采集（M28 插件数据源）：把达到档位的事件落表，api 侧插件按需消费。
/// 幂等：UNIQUE(user_id, torrent_id, hours) + ON CONFLICT DO NOTHING。
async fn collect_milestones(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        INSERT INTO seed_milestones (id, user_id, torrent_id, hours)
        SELECT nextval('seed_milestones_id_seq'), user_id, torrent_id, h.hours
        FROM snatches s
        CROSS JOIN (VALUES (24), (168), (720), (2160)) AS h(hours)
        WHERE s.seeding
          AND EXTRACT(EPOCH FROM (now() - s.completed_at))::bigint / 3600 >= h.hours
          AND s.completed_at IS NOT NULL
        ON CONFLICT (user_id, torrent_id, hours) DO NOTHING
        "#,
    )
    .execute(db)
    .await?;
    if res.rows_affected() > 0 {
        tracing::info!(n = res.rows_affected(), "seed milestones collected");
    }
    Ok(res.rows_affected())
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
                if let Err(e) = collect_milestones(&db).await { tracing::error!(?e, "collect_milestones"); }
            }
            _ = hour_tick.tick() => {
                if first_hour { first_hour = false; continue; }
                if let Err(e) = seeding_reward(&db, 10).await { tracing::error!(?e, "seeding_reward"); }
            }
        }
    }
}

/// 促销计费倍率（M06 倍率表，与 api domain::PromotionKind::multipliers 同口径）
fn billing_multipliers(torrent_kind: Option<&str>, global_kind: Option<&str>) -> (f64, f64) {
    let strength = |k: &str| -> u8 {
        match k {
            "p30" => 1,
            "half" => 2,
            "free" => 3,
            "x2" => 4,
            "x2half" => 5,
            "x2free" => 6,
            _ => 0,
        }
    };
    let table = |k: Option<&str>| -> (f64, f64) {
        match k {
            Some("free") => (1.0, 0.0),
            Some("x2") => (2.0, 1.0),
            Some("x2free") => (2.0, 0.0),
            Some("half") => (1.0, 0.5),
            Some("x2half") => (2.0, 0.5),
            Some("p30") => (1.0, 0.3),
            _ => (1.0, 1.0),
        }
    };
    let winner = match (torrent_kind, global_kind) {
        (Some(t), Some(g)) => Some(if strength(t) >= strength(g) { t } else { g }),
        (t, g) => t.or(g),
    };
    table(winner)
}

#[cfg(test)]
mod tests {
    use super::billing_multipliers;

    #[test]
    fn free_zeroes_download() {
        assert_eq!(billing_multipliers(Some("free"), None), (1.0, 0.0));
    }

    #[test]
    fn stronger_promotion_wins() {
        assert_eq!(billing_multipliers(Some("free"), Some("x2")), (2.0, 1.0));
        assert_eq!(
            billing_multipliers(Some("x2free"), Some("free")),
            (2.0, 0.0)
        );
    }

    #[test]
    fn none_is_normal() {
        assert_eq!(billing_multipliers(None, None), (1.0, 1.0));
    }
}
