//! 闲置账号休眠（0072 建 dormant_mark；0492 对齐公示口径拆出本件）。
//! 从 achv_dormant.rs 按域拆出（300 行门禁）。

use sqlx::PgPool;

/// 闲置账号停用（0072 建；0492 对齐公示口径，2026-11-17 站长拍板方向 A）：
/// 0135 FAQ/规则公示「普通账号连续 150 天未登录可能被回收；无任何流量的
/// 账号回收期更短；回收前邮件+站内信提醒」——本任务即该承诺的执行体。
/// 三键可调（0492）：inactivity_days（缺省 150）/ inactivity_zero_traffic_days
/// （缺省 90，上传下载全 0 适用更短档）/ inactivity_warn_days（缺省 14）。
/// 命中 → dormant_at 打标；不改 status、不删数据，登录侧拦截（login.rs）。
/// 0228 三档：inactivity_policy = mark（缺省，仅打标）| demote（打标+降最低
/// 成长级，数据保留）| archive（打标+archived，档案页隐藏 = 公示「回收」）。
/// 豁免：职员（class>=90）、贵宾 20/养老组 21（0056 身份档=「高等级账号
/// 长期保留」）、捐赠者、在做种者、当月保种契约者、已打标者（幂等）。
/// 打标前 inactivity_warn_days 天发邮件（mail_outbox dedupe 幂等）+站内信。
pub(crate) async fn dormant_mark(db: &PgPool) -> anyhow::Result<u64> {
    let policy: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'inactivity_policy'), 'mark')",
    )
    .fetch_one(db)
    .await
    .unwrap_or_else(|_| "mark".into());
    let days: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'inactivity_days'), 150)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(150);
    let zero_days: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'inactivity_zero_traffic_days'), 90)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(90);
    let warn_days: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'inactivity_warn_days'), 14)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(14);

    // 预提醒（公示「回收前提醒」承诺）：距阈值 warn_days 内未提醒过者。
    // 豁免族同打标口径（身份档 20+ 不提醒）。邮件 dedupe 幂等（0404）。
    let _ = sqlx::query(
        r#"
        INSERT INTO mail_outbox (user_id, to_email, subject, body, dedupe_key)
        SELECT u.id, u.email,
               '账号休眠提醒：请保持登录与做种',
               '您的账号已超过 ' || $1::bigint ||
               ' 天未登录。按站点账号保留策略，长期不活跃的账号可能被休眠停用（登录将受限制）。保持定期登录或做种即可避免。',
               'inactivity_warn:' || u.id
        FROM users u
        WHERE u.class_id < 20 AND NOT u.donor AND u.dormant_at IS NULL
          AND u.status < 2
          AND COALESCE(u.last_seen_at, u.created_at)
            < now() - ($1::bigint * interval '1 day')
          AND COALESCE(u.last_seen_at, u.created_at)
            >= now() - (($1::bigint + 3) * interval '1 day')
          AND NOT EXISTS (SELECT 1 FROM snatches s
                           WHERE s.user_id = u.id AND s.seeding)
          AND NOT EXISTS (SELECT 1 FROM mail_outbox m
                           WHERE m.dedupe_key = 'inactivity_warn:' || u.id)
        "#,
    )
    .bind(warn_days)
    .execute(db)
    .await;
    let _ = sqlx::query(
        r#"
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, u.id,
               '账号休眠提醒',
               '您的账号已超过 ' || $1::bigint ||
               ' 天未登录。按站点账号保留策略，长期不活跃的账号可能被休眠停用（登录将受限制）。保持定期登录或做种即可避免。'
        FROM users u
        WHERE u.class_id < 20 AND NOT u.donor AND u.dormant_at IS NULL
          AND u.status < 2
          AND COALESCE(u.last_seen_at, u.created_at)
            < now() - ($1::bigint * interval '1 day')
          AND COALESCE(u.last_seen_at, u.created_at)
            >= now() - (($1::bigint + 3) * interval '1 day')
          AND NOT EXISTS (SELECT 1 FROM snatches s
                           WHERE s.user_id = u.id AND s.seeding)
          AND NOT EXISTS (SELECT 1 FROM messages m
                           WHERE m.receiver_id = u.id
                             AND m.subject = '账号休眠提醒')
        "#,
    )
    .bind(warn_days)
    .execute(db)
    .await;

    // 打标：一般档（inactivity_days）或零流量档（更短，公示「回收期更短」）。
    // 豁免：身份档 20/21 与职员 90+（0056 累进——「高等级账号长期保留」）。
    let res = sqlx::query(
        r#"
        UPDATE users u SET dormant_at = now()
        WHERE u.class_id < 20 AND NOT u.donor AND u.dormant_at IS NULL
          AND u.status < 2
          AND u.created_at < now() - ($1::bigint * interval '1 day')
          AND (
            COALESCE(u.last_seen_at, u.created_at)
              < now() - ($1::bigint * interval '1 day')
            OR
            (u.uploaded = 0 AND u.downloaded = 0
             AND COALESCE(u.last_seen_at, u.created_at)
               < now() - ($2::bigint * interval '1 day'))
          )
          AND NOT EXISTS (SELECT 1 FROM snatches s
                           WHERE s.user_id = u.id AND s.seeding)
          -- 0424 契约豁免：当月 open/settled 契约者免休眠——签约即承诺保种。
          AND NOT EXISTS (SELECT 1 FROM seed_contracts c
                           WHERE c.user_id = u.id
                             AND c.month = date_trunc('month',
                                   now())::date
                             AND c.status IN ('open', 'settled'))
        "#,
    )
    .bind(days)
    .bind(zero_days)
    .execute(db)
    .await?;
    if res.rows_affected() > 0 {
        tracing::info!(n = res.rows_affected(), "dormant accounts marked");
    }
    // 档位动作（对已打标者执行；幂等——class_id/archived 条件天然收敛）
    if policy == "demote" {
        // 只降成长档（class_rules 覆盖 0-12）：20 贵宾 / 21 养老组 /
        // 91+ 职务档都是特殊身份，休眠不该把它们洗成 class 1。
        // 旧条件 < 90 连 VIP 一起降（0473 补养老组后同样中招）。
        let demoted = sqlx::query(
            "UPDATE users SET class_id = 1 \
             WHERE dormant_at IS NOT NULL AND class_id > 1 \
             AND class_id < 20",
        )
        .execute(db)
        .await?;
        if demoted.rows_affected() > 0 {
            tracing::info!(
                n = demoted.rows_affected(),
                "dormant demoted to class 1"
            );
        }
    }
    if policy == "archive" {
        let archived = sqlx::query(
            "UPDATE users SET archived = true \
             WHERE dormant_at IS NOT NULL AND NOT archived",
        )
        .execute(db)
        .await?;
        if archived.rows_affected() > 0 {
            tracing::info!(n = archived.rows_affected(), "dormant archived");
        }
    }
    Ok(res.rows_affected())
}
