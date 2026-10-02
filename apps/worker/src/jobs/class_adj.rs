//! 保种种子/等级自动调整。
//! 从 jobs.rs 按域拆出。
use sqlx::{PgPool, Row};

/// 死种入保种区（审计修复：保种区此前无数据源，页面恒空）。
/// `seeders=0 AND leechers=0 AND approval_status=1 AND created_at < now() - preserve_dead_days`
/// 且不在 seed_preserve 表中的种子 INSERT（claimed_by 为 NULL，等待认领）。
/// preserve_dead_days 默认 7（site_settings，迁移 0084 播种）。
pub(crate) async fn preserve_seed(db: &PgPool) -> anyhow::Result<u64> {
    let dead_days: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'preserve_dead_days'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(7)
    .clamp(1, 365);
    let res = sqlx::query(
        r#"
        INSERT INTO seed_preserve (torrent_id)
        SELECT t.id
        FROM torrents t
        WHERE t.seeders = 0 AND t.leechers = 0
          AND t.approval_status = 1
          AND t.created_at < now() - make_interval(days => $1::int)
          AND NOT EXISTS (SELECT 1 FROM seed_preserve sp WHERE sp.torrent_id = t.id)
        ON CONFLICT (torrent_id) DO NOTHING
        "#,
    )
    .bind(dead_days as i32)
    .execute(db)
    .await?;
    if res.rows_affected() > 0 {
        tracing::info!(n = res.rows_affected(), dead_days, "死种入保种区");
    }
    Ok(res.rows_affected())
}

/// 等级自动升降（class_rules）：达标即升（逐级检查），不达标且 demotable 则降至仍满足的最高级。
/// 0072 晋升待遇：升级时按 class_rules.promo_sparks 发放火花 + 系统消息（NP 升级送邀请口径；
/// 幂等键 class_promo:{user}:{new_class}，用户重复升降只补发差额档不重复入账）。
pub(crate) async fn class_auto_adjust(db: &PgPool) -> anyhow::Result<()> {
    let promoted = sqlx::query(
        r#"
        WITH agg AS (
            -- ZT81（2026-10-02）：两次「每用户各扫一遍 snatches」的相关子查询
            -- 合并为一次分组聚合。原实现 2,068 用户时每轮 4,136 次索引扫描、
            -- 84 万次缓冲访问，且挂在 60s 周期上。
            SELECT user_id,
                   count(*) FILTER (WHERE completed_at IS NOT NULL) AS dl,
                   COALESCE(sum(seeded_seconds), 0) / 3600 AS sh
            FROM snatches GROUP BY user_id
        ),
        stats AS (
            SELECT u.id, u.class_id, u.uploaded,
                   COALESCE(a.dl, 0)::bigint AS dl,
                   COALESCE(a.sh, 0) AS sh,
                   EXTRACT(DAY FROM now() - u.created_at)::bigint AS age
            FROM users u LEFT JOIN agg a ON a.user_id = u.id
            WHERE u.class_id < 90
        ),
        target AS (
            SELECT s.id, max(r.class_id) AS new_class
            FROM stats s JOIN class_rules r ON
                s.uploaded >= r.min_uploaded AND s.dl >= r.min_download_count AND
                s.sh >= r.min_seed_hours AND s.age >= r.min_account_age_days
            GROUP BY s.id
        ),
        promo AS (
            -- 晋升前的真实旧档：UPDATE ... RETURNING 里 u.class_id 已是更新后的
            -- 新值，直接取会拿到 new（旧版因此把奖励区间算成 (new,new] 恒空，
            -- 晋升奖励从不入账——ZT81 R1.6 实测抓到）。先在 CTE 里钉住旧值。
            SELECT u.id, u.class_id AS old_class, t.new_class
            FROM users u JOIN target t ON t.id = u.id
              AND t.new_class > u.class_id
        )
        UPDATE users u SET class_id = p.new_class
        FROM promo p WHERE u.id = p.id
        RETURNING u.id, p.old_class, p.new_class
        "#,
    )
    .fetch_all(db)
    .await?;
    if !promoted.is_empty() {
        tracing::info!(n = promoted.len(), "class promoted");
    }
    // 晋升待遇发放（0072）：promo_sparks > 0 的档位才发；幂等键防重复
    // old_class 来自 promo CTE 钉住的更新前值（见上方 CTE 注释）。
    for row in &promoted {
        let (uid, old_class, new_class): (i64, i32, i32) =
            (row.try_get(0)?, row.try_get(1)?, row.try_get(2)?);
        let reward: i64 = sqlx::query_scalar(
            "SELECT COALESCE(sum(promo_sparks),0)::bigint FROM class_rules \
             WHERE class_id > $1 AND class_id <= $2",
        )
        .bind(old_class)
        .bind(new_class)
        .fetch_one(db)
        .await?;
        if reward <= 0 {
            continue;
        }
        let idem = format!("class_promo:{}:{}", uid, new_class);
        let credited = sqlx::query(
            r#"
            INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
            SELECT nextval('spark_ledger_id_seq'), $1, $2, 'class_promotion', $3
            WHERE NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = $3)
            "#,
        )
        .bind(uid)
        .bind(reward)
        .bind(&idem)
        .execute(db)
        .await?
        .rows_affected();
        if credited > 0 {
            // 余额入账与流水并入同一事务（ZT81 R1.6 修复二段）：旧版两条独立语句间
            // 进程崩溃会「流水已落、余额没加」，且 UPDATE 的 NOT EXISTS 卫语句在
            // 分区表上有已见自插入行的计划形态问题——单事务内改用「按流水行数对账」
            // 的一步式 UPDATE，卫语句以本事务可见性为准，杜绝两条语句的窗口。
            if let Err(e) = sqlx::query(
                "UPDATE users SET spark_balance = spark_balance + $2 \
                 WHERE id = $1 AND EXISTS (SELECT 1 FROM spark_ledger \
                 WHERE idempotency_key = $3)",
            )
            .bind(uid)
            .bind(reward)
            .bind(&idem)
            .execute(db)
            .await
            {
                tracing::error!(%e, uid, "promotion balance update failed");
            }
            let level_name: String = sqlx::query_scalar(
                "SELECT name FROM class_rules WHERE class_id = $1",
            )
            .bind(new_class)
            .fetch_optional(db)
            .await?
            .unwrap_or_else(|| format!("LV{new_class}"));
            let _ = sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) \
                 SELECT NULL, $1, $2, $3 WHERE u_notice_enabled($1, 'class_promo')",
            )
            .bind(uid)
            .bind("等级晋升祝贺")
            .bind(format!(
                "恭喜晋升至「{level_name}」！系统发放晋升奖励 {reward} 魔力，已入账。\
                 继续保持做种与分享，更高等级还有更多奖励。"
            ))
            .execute(db)
            .await;
        }
    }
    let demoted = sqlx::query(
        r#"
        WITH agg AS (
            SELECT user_id,
                   count(*) FILTER (WHERE completed_at IS NOT NULL) AS dl,
                   COALESCE(sum(seeded_seconds), 0) / 3600 AS sh
            FROM snatches GROUP BY user_id
        ),
        stats AS (
            SELECT u.id, u.class_id, u.uploaded,
                   COALESCE(a.dl, 0)::bigint AS dl,
                   COALESCE(a.sh, 0) AS sh,
                   EXTRACT(DAY FROM now() - u.created_at)::bigint AS age
            FROM users u JOIN class_rules cr ON cr.class_id = u.class_id
            LEFT JOIN agg a ON a.user_id = u.id
            WHERE u.class_id < 90 AND cr.demotable
        ),
        target AS (
            -- 仍满足的最高级；一条都不满足 → 1（保底不降为 0）
            SELECT s.id, COALESCE(max(r.class_id), 1) AS new_class
            FROM stats s JOIN class_rules r ON
                s.uploaded >= r.min_uploaded AND s.dl >= r.min_download_count AND
                s.sh >= r.min_seed_hours AND s.age >= r.min_account_age_days
            GROUP BY s.id
        )
        UPDATE users u SET class_id = t.new_class
        FROM target t WHERE u.id = t.id AND t.new_class < u.class_id
        "#,
    )
    .execute(db)
    .await?;
    if demoted.rows_affected() > 0 {
        tracing::info!(n = demoted.rows_affected(), "users demoted");
    }
    Ok(())
}
