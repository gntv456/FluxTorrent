//! 成就发放/休眠标记。
//! 从 jobs.rs 按域拆出。

use sqlx::PgPool;

/// 成就授予（0079 G6，U3D 口径教育站收敛版）：四族指标聚合 → 达标授予 + 火花奖励。
/// 幂等：PK (user_id, def_id) 天然防重；奖励走幂等键 achievement:{def_code}:{uid}。
pub(crate) async fn achievement_grant(db: &PgPool) -> anyhow::Result<u64> {
    let _res = sqlx::query(
        r#"
        WITH metrics AS (
            SELECT u.id AS user_id,
                   COALESCE(u.seeding_size, 0) AS seeding_bytes,
                   (SELECT count(*) FROM resurrections r
                      WHERE r.user_id = u.id
                        AND r.status = 'done') AS rescue_count,
                   (SELECT count(*) FROM torrents t
                      WHERE t.owner_id = u.id
                        AND t.approval_status = 1) AS upload_count,
                   (SELECT count(*) FROM posts p
                      WHERE p.user_id = u.id) AS post_count,
                   (SELECT count(*) FROM subtitles sb
                      WHERE sb.user_id = u.id AND sb.deleted_at IS NULL
                        AND sb.status = 1 AND NOT sb.anon) AS subtitle_count
            FROM users u WHERE u.status < 2
        ),
        m AS (
            SELECT user_id, 'seeding_bytes' AS metric,
                    seeding_bytes AS val FROM metrics
            UNION ALL SELECT user_id, 'rescue_count', rescue_count FROM metrics
            UNION ALL SELECT user_id, 'upload_count', upload_count FROM metrics
            UNION ALL SELECT user_id, 'post_count', post_count FROM metrics
            UNION ALL SELECT user_id, 'subtitle_count',
                      subtitle_count FROM metrics
        ),
        due AS (
            SELECT m.user_id, d.id AS def_id, d.code, d.reward_sparks, m.val
            FROM m JOIN achievement_defs d
              ON d.metric = m.metric AND m.val >= d.threshold
        )
        INSERT INTO user_achievements (user_id, def_id, metric_value)
        SELECT user_id, def_id, val FROM due
        ON CONFLICT (user_id, def_id) DO NOTHING
        "#,
    )
    .fetch_all(db)
    .await?;
    // RETURNING 只能引用目标表列（code/reward_sparks 属于 achievement_defs）。
    // 0079 上线以来因 RETURNING 语法错误，成就系统从未授予过 —— 改为插入后反查达标行。
    let res: Vec<(i64, i64, String, i64, i64)> = sqlx::query_as(
        r#"
        SELECT ua.user_id, ua.def_id, d.code, d.reward_sparks, ua.metric_value
        FROM user_achievements ua
        JOIN achievement_defs d ON d.id = ua.def_id
        WHERE (ua.user_id, ua.def_id) IN (
            SELECT m.user_id, d2.id FROM (
                SELECT u.id AS user_id,
                       COALESCE(u.seeding_size, 0) AS seeding_bytes,
                       (SELECT count(*) FROM resurrections r
                      WHERE r.user_id = u.id
                        AND r.status = 'done') AS rescue_count,
                       (SELECT count(*) FROM torrents t
                      WHERE t.owner_id = u.id
                        AND t.approval_status = 1) AS upload_count,
                       (SELECT count(*) FROM posts p
                      WHERE p.user_id = u.id) AS post_count,
                       (SELECT count(*) FROM subtitles sb
                      WHERE sb.user_id = u.id AND sb.deleted_at IS NULL
                        AND sb.status = 1 AND NOT sb.anon) AS subtitle_count
                FROM users u WHERE u.status < 2
            ) m JOIN achievement_defs d2 ON (
                (d2.metric = 'seeding_bytes'
                  AND m.seeding_bytes >= d2.threshold) OR
                (d2.metric = 'rescue_count'
                  AND m.rescue_count >= d2.threshold) OR
                (d2.metric = 'upload_count'
                  AND m.upload_count >= d2.threshold) OR
                (d2.metric = 'post_count'
                  AND m.post_count >= d2.threshold) OR
                (d2.metric = 'subtitle_count'
                  AND m.subtitle_count >= d2.threshold))
        )
        "#,
    )
    .fetch_all(db)
    .await?;
    for &(uid, def_id, ref code, reward, _val) in &res {
        if reward > 0 {
            let idem = format!("achievement:{code}:{uid}");
            // 审计修复（原子性）：INSERT 流水与 UPDATE 余额包进同一事务——
            // 两语句分离时中途崩溃会出现「流水已落、余额未加」（或反之），对账永久撕裂。
            // 幂等键护栏保持：事务内 NOT EXISTS 防任务重跑双发。
            let mut tx = db.begin().await?;
            sqlx::query(
                r#"
                INSERT INTO spark_ledger
                    (id, user_id, amount, kind, idempotency_key)
                SELECT nextval('spark_ledger_id_seq'), $1, $2, 'achievement', $3
                WHERE NOT EXISTS (SELECT 1 FROM spark_ledger
                  WHERE idempotency_key = $3)
                "#,
            )
            .bind(uid)
            .bind(reward)
            .bind(&idem)
            .execute(&mut *tx)
            .await?;
            // 审计修复（幂等）：UPDATE 与 INSERT 的 NOT EXISTS 同护栏 ——
            // 否则每小时任务重跑时流水幂等跳过、余额却再加一次（每用户每小时白得 200）
            sqlx::query(
                "UPDATE users SET spark_balance = spark_balance + $2 \
                 WHERE id = $1 \
                 AND NOT EXISTS (SELECT 1 FROM spark_ledger
                  WHERE idempotency_key = $3)",
            )
            .bind(uid)
            .bind(reward)
            .bind(&idem)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
        }
        let _ = def_id;
    }
    if !res.is_empty() {
        tracing::info!(n = res.len(), "achievements grant pass done");
    }
    // 新授予（本轮才落 user_achievements 的行）补发站内信；历史达标行只走上面的
    // 幂等发奖路径，不重复发信（修复每小时向全部历史达标成就重发通知的轰炸）。
    let newly: Vec<(i64, String, i64)> = sqlx::query_as(
        r#"
        SELECT ua.user_id, d.code, d.reward_sparks
        FROM user_achievements ua
        JOIN achievement_defs d ON d.id = ua.def_id
        WHERE (ua.user_id, ua.def_id) IN (
            SELECT m.user_id, d2.id FROM (
                SELECT u.id AS user_id,
                       COALESCE(u.seeding_size, 0) AS seeding_bytes,
                       (SELECT count(*) FROM resurrections r
                      WHERE r.user_id = u.id
                        AND r.status = 'done') AS rescue_count,
                       (SELECT count(*) FROM torrents t
                      WHERE t.owner_id = u.id
                        AND t.approval_status = 1) AS upload_count,
                       (SELECT count(*) FROM posts p
                      WHERE p.user_id = u.id) AS post_count,
                       (SELECT count(*) FROM subtitles sb
                      WHERE sb.user_id = u.id AND sb.deleted_at IS NULL
                        AND sb.status = 1 AND NOT sb.anon) AS subtitle_count
                FROM users u WHERE u.status < 2
            ) m JOIN achievement_defs d2 ON (
                (d2.metric = 'seeding_bytes'
                  AND m.seeding_bytes >= d2.threshold) OR
                (d2.metric = 'rescue_count'
                  AND m.rescue_count >= d2.threshold) OR
                (d2.metric = 'upload_count'
                  AND m.upload_count >= d2.threshold) OR
                (d2.metric = 'post_count'
                  AND m.post_count >= d2.threshold) OR
                (d2.metric = 'subtitle_count'
                  AND m.subtitle_count >= d2.threshold))
        )
        AND NOT EXISTS (
            SELECT 1 FROM messages msg
            WHERE msg.receiver_id = ua.user_id
              AND msg.subject = '成就达成'
              AND msg.body LIKE '%「' || d.code || '」%'
        )
        "#,
    )
    .fetch_all(db)
    .await?;
    for &(uid, ref code, reward) in &newly {
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, \
             subject, body) VALUES (NULL, $1, $2, $3)",
        )
        .bind(uid)
        .bind("成就达成")
        .bind(format!(
            "恭喜达成成就「{code}」！奖励 {reward} 魔力已入账。"
        ))
        .execute(db)
        .await;
    }
    if !newly.is_empty() {
        tracing::info!(n = newly.len(), "achievements granted");
    }
    Ok(res.len() as u64)
}

/// 闲置账号停用（0072，U3D AutoDisableInactiveUsers 口径的教育站收敛版）：
/// 90 天未登录、无任何做种、非员工（class<90）且非捐赠者 → dormant_at 打标。
/// 不改 status（保留封禁语义）、不删数据；登录侧拦截 dormant_at 非空者并提示联系管理组。
/// 排除 dormant_at 已打标（幂等）与 90 天内注册的新号（新人宽限）。
pub(crate) async fn dormant_mark(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        UPDATE users u SET dormant_at = now()
        WHERE u.class_id < 90 AND NOT u.donor AND u.dormant_at IS NULL
          AND u.created_at < now() - interval '90 days'
          AND COALESCE(u.last_seen_at, u.created_at)
            < now() - interval '90 days'
          AND NOT EXISTS (SELECT 1 FROM snatches s
                           WHERE s.user_id = u.id AND s.seeding)
        "#,
    )
    .execute(db)
    .await?;
    if res.rows_affected() > 0 {
        tracing::info!(n = res.rows_affected(), "dormant accounts marked");
    }
    Ok(res.rows_affected())
}
