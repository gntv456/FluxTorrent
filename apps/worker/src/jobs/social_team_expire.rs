//! 组队契约到期清理。
//! 从 jobs.rs 按域拆出。

use sqlx::PgPool;

/// 组队契约超时失败（0103）：到期仍未达成 → 判失败。
///
/// 设计口径（见 `_doc/契约失败流转与信誉.md`）：
///   * **不连坐**：失败是团队的共同结果，不额外惩罚个别成员；只记一次失败事实（信誉小减）
///   * 与成功结算互斥：本 job 排在 social_team_settle **之后**执行，恰好卡在期限内达标的仍算成功；
///     事务内再用 `status IN (0,1) AND settled_at IS NULL` 做 CAS，防止被结算抢先
///   * 与「中途退出」区分：到期未达标**不算逃跑**，不套用退出惩罚——惩罚只针对主动跑掉的人
pub(crate) async fn social_team_expire(db: &PgPool) -> anyhow::Result<u64> {
    let rep_delta: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'social_rep_on_failed'), -5)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(-5);
    let rep_min: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'social_rep_min'), 0)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let rep_max: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'social_rep_max'), 2000)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(2000);

    let due: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM social_team \
         WHERE status IN (0, 1) AND settled_at IS NULL \
           AND deadline_at IS NOT NULL AND deadline_at < now()",
    )
    .fetch_all(db)
    .await?;

    let mut n = 0u64;
    for team_id in &due {
        let mut tx = db.begin().await?;
        // CAS：与成功结算互斥（结算 job 排在前面，已把 settled_at 置位的队伍在此被跳过）
        let claimed = sqlx::query(
            "UPDATE social_team SET status = 3, fail_reason = 'deadline_exceeded', settled_at = now() \
             WHERE id = $1 AND status IN (0, 1) AND settled_at IS NULL",
        )
        .bind(team_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if claimed == 0 {
            continue;
        }

        let tid: Option<i64> = sqlx::query_scalar(
            "SELECT torrent_id FROM resurrections WHERE team_id = $1 LIMIT 1",
        )
        .bind(team_id)
        .fetch_optional(&mut *tx)
        .await?;

        sqlx::query(
            "UPDATE resurrections SET status = 'expired', finished_at = now() \
             WHERE team_id = $1 AND status = 'open'",
        )
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

        let members: Vec<i64> = sqlx::query_scalar(
            "SELECT uid FROM social_team_member WHERE team_id = $1 \
             AND join_status IN (0, 1)",
        )
        .bind(team_id)
        .fetch_all(&mut *tx)
        .await?;

        for uid in &members {
            sqlx::query(
                "INSERT INTO social_reputation (uid, score, failed_count, updated_at) \
                 VALUES ($1, 1000 + $2, 1, now()) \
                 ON CONFLICT (uid) DO UPDATE SET \
                   score = LEAST($3, GREATEST($4, social_reputation.score + $2)), \
                   failed_count = social_reputation.failed_count + 1, \
                   updated_at = now()",
            )
            .bind(*uid)
            .bind(rep_delta)
            .bind(rep_max)
            .bind(rep_min)
            .execute(&mut *tx)
            .await?;

            // 文案只陈述事实 + 说明已产生的收益不受影响，不指责
            // （协作失败本就令人沮丧，不该再加羞辱）
            let body = match tid {
                Some(t) => format!(
                    "资源 #{t} 的协作保种未在期限内达标，契约已结束。\
                     你已产生的做种时长仍计入做种收益，不受影响。"
                ),
                None => "协作保种未在期限内达标，契约已结束。".to_string(),
            };
            let _ = sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, \
                 subject, body) VALUES (NULL, $1, $2, $3)",
            )
            .bind(*uid)
            .bind("保种协作已到期")
            .bind(body)
            .execute(&mut *tx)
            .await;
        }

        tx.commit().await?;
        n += 1;
    }

    if n > 0 {
        tracing::info!(n, "social teams expired");
    }
    Ok(n)
}

/// 教材愿望单推送（0074，U3D WishList 教育化）：扫过去 1 小时过审的种子，
/// 对 wishlist 做 ILIKE/维度匹配；每条愿望 24h 限推一次，**每用户聚合一封信**（防信箱轰炸）。
pub(crate) async fn wishlist_notify(db: &PgPool) -> anyhow::Result<u64> {
    // 审计修复（原子性）：INSERT 消息与 UPDATE notified_at 包进同一事务——
    // 两条独立语句中途崩溃会出现「发了信却没标记」（下轮重发）或「标记了却没发信」
    // （该愿望 24h 内彻底漏推）的错位。事务内两语句同生共死。
    let mut tx = db.begin().await?;
    let res = sqlx::query(
        r#"
        WITH recent AS (
            SELECT id, name FROM torrents
            WHERE approval_status = 1
              AND approved_at > now() - interval '1 hour'
              AND approved_at IS NOT NULL
        ),
        hits AS (
            SELECT w.id AS wish_id, w.user_id, r.id AS torrent_id, r.name AS torrent_name
            FROM wishlist w
            JOIN recent r ON r.name ILIKE '%' || w.keyword || '%'
            WHERE (w.category_id IS NULL OR w.category_id = (SELECT category_id FROM torrents t WHERE t.id = r.id))
              AND (w.notified_at IS NULL OR w.notified_at < now() - interval '24 hours')
        ),
        agg AS (
            SELECT user_id, string_agg('#' || torrent_id || ' ' || torrent_name, E'
' ORDER BY torrent_id) AS body,
                   bool_or(wish_id IS NOT NULL) AS dummy
            FROM hits GROUP BY user_id
        )
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, user_id, '愿望单命中：你关注的新资源已上架', '你订阅的关键词有新种子过审：

' || body || '

（每条愿望 24 小时内只提醒一次；可在「我的 → 愿望单」管理订阅）'
        FROM agg
        WHERE dummy
        RETURNING 1
        "#,
    )
    .fetch_all(&mut *tx)
    .await?;
    // 24h 节流推进（审计修复：原第二条独立 UPDATE 引用上一条语句的 CTE `recent`，
    // 每轮报 relation "recent" does not exist，notified_at 永不推进 → 命中窗口内每小时重发。
    // 改为同一事务内重算同构 CTE 后推进，与 INSERT 同生共死，杜绝"发了信却没标记"的错位）
    let _ = sqlx::query(
        r#"
        WITH recent AS (
            SELECT id, name FROM torrents
            WHERE approval_status = 1
              AND approved_at > now() - interval '1 hour'
              AND approved_at IS NOT NULL
        ),
        hits AS (
            SELECT w.id AS wish_id, w.user_id
            FROM wishlist w
            JOIN recent r ON r.name ILIKE '%' || w.keyword || '%'
            WHERE (w.category_id IS NULL OR w.category_id = (SELECT category_id FROM torrents t WHERE t.id = r.id))
              AND (w.notified_at IS NULL OR w.notified_at < now() - interval '24 hours')
        )
        UPDATE wishlist w SET notified_at = now()
        WHERE w.id IN (SELECT wish_id FROM hits)
        "#,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    if !res.is_empty() {
        tracing::info!(n = res.len(), "wishlist notifications sent");
    }
    Ok(res.len() as u64)
}

/// 做种里程碑采集（M28 插件数据源）：把达到档位的事件落表，api 侧插件按需消费。
/// 幂等：UNIQUE(user_id, torrent_id, hours) + ON CONFLICT DO NOTHING。
pub(crate) async fn collect_milestones(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        INSERT INTO seed_milestones (id, user_id, torrent_id, hours)
        SELECT nextval('seed_milestones_id_seq'), user_id, torrent_id, h.hours
        FROM snatches s
        CROSS JOIN (VALUES (24), (168), (720), (2160)) AS h(hours)
        WHERE s.seeding
          -- 审计修复：档位判定改用累计做种秒数（与 H&R/seeding_reward 同口径）。
          -- 旧墙钟口径「完成至今的挂机时长」会把只下载不做种的账号也计入里程碑。
          AND s.seeded_seconds >= h.hours * 3600
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
