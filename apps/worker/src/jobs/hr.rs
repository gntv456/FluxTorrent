//! H&R 执法与惩罚。
//! 从 jobs.rs 按域拆出。

use sqlx::PgPool;

/// H&R 追责（M05 补齐）：为「完成下载」建立策略快照（时点正确），到期结算违规。
/// 策略口径（§5.4）：hr_policy JSONB {"days": N, "seed_hours": H} —— 完成后 N 天内需累计做种 H 小时。
/// B-01：完成时刻正处免费（free/x2free，含全局站免）窗口的种子豁免 H&R —— 行业惯例。
pub(crate) async fn hr_enforce(db: &PgPool) -> anyhow::Result<()> {
    // 1) 为新完成的下载建快照（幂等）；免费窗口内完成的不建快照（豁免）
    sqlx::query(
        r#"
        INSERT INTO hr_snapshots (user_id, torrent_id, required_seconds, deadline)
        SELECT s.user_id, s.torrent_id,
               COALESCE((t.hr_policy->>'seed_hours')::int, 48) * 3600,
               s.completed_at + make_interval(days => COALESCE((t.hr_policy->>'days')::int, 14))
        FROM snatches s
        JOIN torrents t ON t.id = s.torrent_id
        WHERE s.completed_at IS NOT NULL
          AND NOT EXISTS (SELECT 1 FROM hr_snapshots h WHERE h.user_id = s.user_id AND h.torrent_id = s.torrent_id)
          AND COALESCE(t.hr_policy->>'enabled', 'true')::boolean
          -- 0072 buffer 豁免（U3D hitrun.buffer 口径）：下载量不足种子 10% 视为误触/秒删，不计 H&R
          AND s.downloaded > t.size * 104 / 1000   -- 与券核销同阈值（≈10.4%）：10%~10.4% 区间不再误判
          AND NOT EXISTS (
              SELECT 1 FROM promotions p
              WHERE (p.torrent_id = s.torrent_id
                     OR (p.torrent_id IS NULL AND (
                         p.scope = 'global'
                         OR (p.scope = 'official' AND t.official_tag)
                         OR (p.scope = 'non_official' AND NOT t.official_tag)
                         OR (p.scope = 'category' AND t.category_id = p.category_id))))
                AND p.starts_at <= s.completed_at AND p.ends_at > s.completed_at
                AND p.kind IN ('free', 'x2free')
          )
          -- 保种员 / VIP 持 hr.exempt 权限 → 免除 H&R，不建快照
          AND NOT user_can(s.user_id, 'hr.exempt')
        ON CONFLICT DO NOTHING
        "#,
    )
    .execute(db)
    .await?;

    // 2) 刷新累计做种秒数（快照口径：snatches.seeded_seconds）
    sqlx::query(
                "UPDATE hr_snapshots h SET seeded_seconds = s.seeded_seconds, \
         updated_at = now() FROM snatches s WHERE s.user_id = h.user_id AND s.torrent_id = h.torrent_id AND h.status = 'open'",
    )
    .execute(db)
    .await?;

    // 3) 达标即 satisfied
    sqlx::query(
                "UPDATE hr_snapshots SET status = 'satisfied', \
         updated_at = now() WHERE status = 'open' AND seeded_seconds >= required_seconds",
    )
    .execute(db)
    .await?;

    // 3.5) 预警（0072，U3D prewarn 口径）：48h 内到期、未达标、未预警过的 → 站内信提醒。
    //      处罚前的缓冲带：教育站新人多，一次 PM 能挡掉大部分无意违规。
    //      0227 参数化：提前量读 hr_prewarn_hours 设定键（0=关闭整段；宽进制
    //      模板 48h、淘汰制 24h）；收件人尊重 notice_prefs.hr_prewarn 开关。
    let prewarn_hours: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'hr_prewarn_hours')::bigint, 0)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let prewarned = if prewarn_hours > 0 {
        sqlx::query(
            r#"
            WITH due AS (
                UPDATE hr_snapshots SET prewarned_at = now(), updated_at = now()
                WHERE status = 'open' AND prewarned_at IS NULL
                  AND seeded_seconds < required_seconds
                  AND deadline < now() + make_interval(hours => $1)
                RETURNING user_id, torrent_id, seeded_seconds, required_seconds, deadline
            )
            INSERT INTO messages (sender_id, receiver_id, subject, body)
            SELECT NULL, d.user_id,
                   'H&R 预警：请尽快补足做种',
                   format('你完成的种子 #%s 距 H&R 考察截止还剩不到 %s 小时（截止 %s）。当前累计做种 %s 小时，'
                          '需 %s 小时。请尽快恢复做种；也可在「我的 H&R」页用魔力自助免罪。',
                          d.torrent_id,
                          $2,
                          to_char(d.deadline AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM-DD HH24:MI'),
                          round(d.seeded_seconds / 3600.0, 1),
                          round(d.required_seconds / 3600.0, 1))
            FROM due d
            JOIN users u ON u.id = d.user_id
            WHERE COALESCE((u.notice_prefs->>'hr_prewarn')::boolean, true)
            "#,
        )
        .bind(prewarn_hours)
        .bind(prewarn_hours)
        .execute(db)
        .await?
    } else {
        sqlx::query("SELECT 1 WHERE false").execute(db).await?
    };
    if prewarned.rows_affected() > 0 {
        tracing::info!(n = prewarned.rows_affected(), "H&R pre-warnings sent");
    }

    // 4) 过期未达标 → violated + 落违规表（追责依据）
    let violated = sqlx::query(
        r#"
        WITH dead AS (
            UPDATE hr_snapshots SET status = 'violated', updated_at = now()
            WHERE status = 'open' AND deadline < now()
            RETURNING user_id, torrent_id, seeded_seconds, required_seconds
        ),
        ins AS (
            INSERT INTO hr_violations (user_id, torrent_id, seeded_seconds, required_seconds)
            SELECT user_id, torrent_id, seeded_seconds, required_seconds FROM dead
            ON CONFLICT DO NOTHING
            RETURNING user_id, torrent_id, seeded_seconds, required_seconds
        )
        -- 审计修复（P1）：violated 此前只落表+日志零成本躺平。补 PM 告知违规与免罪途径
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, ins.user_id, 'H&R 违规确认',
               '种子 #' || ins.torrent_id || ' 的 H&R 考察期已结束且未达标（做种 '
               || round(ins.seeded_seconds / 3600.0, 1) || ' 小时 / 要求 '
               || round(ins.required_seconds / 3600.0, 1) || ' 小时），已记违规一次。
               持续做种可自行恢复；也可在「我的 H&R」用 20000 魔力自助免罪。累计多次违规将影响下载权限。'
        FROM ins
        "#,
    )
    .execute(db)
    .await?;
    if violated.rows_affected() > 0 {
        tracing::warn!(n = violated.rows_affected(), "H&R violations detected");
        // 违规行同步 snatches.hr_flag（/me/hr 与列表角标口径）
        let _ = sqlx::query(
            "UPDATE snatches s SET hr_flag = TRUE FROM hr_violations v \
             WHERE v.user_id = s.user_id AND v.torrent_id = s.torrent_id AND NOT s.hr_flag",
        )
        .execute(db)
        .await;
    }

    // 5) hr_flag 刷新（0029 一次性迁移的运行时延续）：完成已超 14 天且做种时长 < 120h。
    //    此前该标记只在迁移里置过一次，运行时无人刷新 —— /me/hr（community_http）口径失真。
    sqlx::query(
        // 审计修复（P1）：硬编码 14 天/120 小时与 hr_policy 可配口径脱节，逐种取 policy
        "UPDATE snatches s SET hr_flag = TRUE \
         FROM torrents t WHERE t.id = s.torrent_id AND s.completed_at IS NOT NULL \
           AND s.seeded_seconds < COALESCE((t.hr_policy->>'seed_hours')::int, 48) * 3600 \
           AND s.completed_at < now() - make_interval(days => COALESCE((t.hr_policy->>'days')::int, 14)) \
           AND NOT s.hr_flag",
    )
    .execute(db)
    .await?;
    Ok(())
}

/// H&R 违规处罚执行点（审计修复：hr_enforce 此前只落表+PM 零成本躺平）。
/// 未解决违规数（hr_violations.resolved_at IS NULL）≥ hr_violation_limit（默认 3）
///   → users.download_enabled = false + PM 说明自助免罪路径；
/// 违规数降回阈值以下 → 恢复 download_enabled = true。
/// 口径取舍（注释存档）：是否「因 H&R 被禁」不引入新列/PM 反查——违规数一旦低于阈值
/// 就恢复下载，可能顺带恢复因其他原因（如 Ratio Watch 到期处置）被禁的用户。
/// 取舍理由：Ratio Watch 侧管理组手动恢复是主路径，此处自动恢复保住多数用户的体验；
/// 若需精确归因，可后续为 users 增加 ban 原因位图。豁免 hr.exempt / 员工不处罚。
pub(crate) async fn hr_punish(db: &PgPool) -> anyhow::Result<()> {
    let limit: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'hr_violation_limit'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(3)
    .clamp(1, 100);

    // ① 超限 → 暂停下载 + PM（仅本轮新被禁的发信，幂等靠 download_enabled 翻转）
    let punished = sqlx::query(
        r#"
        WITH viol AS (
            SELECT v.user_id, count(*) AS n
            FROM hr_violations v
            WHERE v.resolved_at IS NULL
            GROUP BY v.user_id
            HAVING count(*) >= $1
        ),
        banned AS (
            UPDATE users u
            SET download_enabled = FALSE
            FROM viol
            WHERE u.id = viol.user_id
              AND u.status < 2
              AND u.download_enabled
              AND NOT user_can(u.id, 'hr.exempt')
            RETURNING u.id, viol.n
        )
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, b.id, '下载权限暂停：H&R 违规超限',
               format('您当前有 %s 条未解决的 H&R 违规（阈值 %s），已暂停下载权限。'
                      '恢复方式：①持续做种达标后违规自动消除；②在「我的 H&R」页用魔力自助免罪；'
                      '③联系管理组申请 Pardon。违规数降回阈值以下后下载权限将自动恢复。',
                      b.n, $1)
        FROM banned b
        "#,
    )
    .bind(limit)
    .execute(db)
    .await?;
    if punished.rows_affected() > 0 {
        tracing::warn!(
            n = punished.rows_affected(),
            limit,
            "H&R 违规超限，已暂停下载权限"
        );
    }

    // ② 降回阈值以下 → 自动恢复下载。
    // 口径注释：不区分当初被禁原因（见函数头取舍说明）——违规数低于阈值即恢复，
    // 极小概率把其他原因禁用的账号一并恢复，换取 H&R 自助闭环不依赖人工。
    let restored = sqlx::query(
        r#"
        UPDATE users u
        SET download_enabled = TRUE
        WHERE u.status < 2
          AND NOT u.download_enabled
          AND COALESCE((SELECT count(*) FROM hr_violations v
                        WHERE v.user_id = u.id AND v.resolved_at IS NULL), 0) < $1
          AND NOT EXISTS (
              -- Ratio Watch 到期处置仍生效的用户不在此恢复（那边由管理组/观察期自愈管理）
              SELECT 1 FROM users u2
              WHERE u2.id = u.id AND u2.ratio_watch_until IS NOT NULL AND u2.ratio_watch_until < now()
          )
        "#,
    )
    .bind(limit)
    .execute(db)
    .await?;
    if restored.rows_affected() > 0 {
        tracing::info!(
            n = restored.rows_affected(),
            "H&R 违规降回阈值以下，已恢复下载权限"
        );
    }
    Ok(())
}
