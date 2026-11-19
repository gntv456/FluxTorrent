//! 等级降级 + 预告信（0498 从 class_adj.rs 拆出：行数门禁）。
//! 升级祝贺信在 class_adj；降级此前零通知（体检 A6 静默降级）。

use sqlx::{PgPool, Row};

pub(super) async fn demote_with_notice(db: &PgPool) -> anyhow::Result<()> {
    let demoted = sqlx::query(
        r#"
        WITH agg AS (
            SELECT user_id,
                   count(*) FILTER (WHERE completed_at IS NOT NULL) AS dl,
                   COALESCE(sum(seeded_seconds), 0) / 3600 AS sh
            FROM snatches GROUP BY user_id
        ),
        seed AS (
            -- 0501 四维：在种体积/颗数现值聚合（与 snatches.seeding 同源）
            SELECT user_id,
                   COALESCE(sum(t.size), 0)::bigint AS sv,
                   count(*)::bigint AS sc
            FROM snatches s JOIN torrents t ON t.id = s.torrent_id
            WHERE s.seeding
            GROUP BY user_id
        ),
        stats AS (
            SELECT u.id, u.class_id, u.uploaded,
                   COALESCE(a.dl, 0)::bigint AS dl,
                   COALESCE(a.sh, 0)::bigint AS sh,
                   u.downloaded,
                   COALESCE(p.sv, 0)::bigint AS sv,
                   COALESCE(p.sc, 0)::bigint AS sc,
                   -- 分享率万分比；downloaded=0 视为∞（与全站 ratio 口径
                   -- 一致：0 下载新号不惩罚）——用最大整数放行所有门槛
                   CASE WHEN u.downloaded = 0 THEN 2147483647
                        ELSE (u.uploaded * 10000
                              / NULLIF(u.downloaded, 0))::int
                   END AS rbp,
                   EXTRACT(DAY FROM now() - u.created_at)::bigint AS age
            FROM users u JOIN class_rules cr ON cr.class_id = u.class_id
            LEFT JOIN agg a ON a.user_id = u.id
            LEFT JOIN seed p ON p.user_id = u.id
            WHERE u.class_id < 90 AND cr.demotable
        ),
        target AS (
            -- P-325：降级地板。字典里 LV0 门槛全零 ⇒「恒被满足」，旧版的
            -- INNER JOIN 让 COALESCE(…, 1) 永远走不到，失责的 LV5 成员会被
            -- 一路降到 class 0（Peasant）——而站内地板是 1：achv_dormant
            --  dormant 降级写 1、subtitles_govern 判 class_id >= 1、
            --  onboarding 的 demotable 开关挂的也是 class 1。
            -- 改 LEFT JOIN 后「只剩 0 档」= 空组 → NULL → 保底落到 1。
            -- 0501 四维谓词：NULL = 不参与（兼容存量，站长按需开维度）。
            SELECT s.id, COALESCE(max(r.class_id), 1) AS new_class
            FROM stats s LEFT JOIN class_rules r ON
                s.uploaded >= r.min_uploaded
                AND s.dl >= r.min_download_count AND
                s.sh >= r.min_seed_hours AND s.age >= r.min_account_age_days AND
                (r.min_downloaded IS NULL
                     OR s.downloaded >= r.min_downloaded) AND
                (r.min_seed_volume IS NULL OR s.sv >= r.min_seed_volume) AND
                (r.min_seed_count IS NULL OR s.sc >= r.min_seed_count) AND
                (r.min_ratio_bp IS NULL OR s.rbp >= r.min_ratio_bp) AND
                r.class_id >= 1
            GROUP BY s.id
        )
        UPDATE users u SET class_id = t.new_class
        FROM target t WHERE u.id = t.id AND t.new_class < u.class_id
        RETURNING u.id, u.class_id AS new_class, u.username
        "#,
    )
    .fetch_all(db)
    .await?;
    // 0498 降级预告（产品体检 A6/10-26 P2「静默降级」）：升级有祝贺信、
    // 降级零通知——用户某天发现等级没了却不知为何。补预告信（事后告知，
    // 带恢复路径），复用升级祝贺信的形状（u_notice_enabled + 提交后发）。
    // 幂等锚：class_demote_notice:{uid}:{new_class}，防重复降级路径重发。
    for r in &demoted {
        let uid: i64 = r.get("id");
        let new_class: i32 = r.get("new_class");
        let username: String = r.get("username");
        let name: Option<String> =
            sqlx::query_scalar("SELECT name FROM user_classes WHERE id = $1")
                .bind(new_class)
                .fetch_one(db)
                .await
                .unwrap_or_else(|_| {
                    Some("LV".to_string() + &new_class.to_string())
                });
        let sent = sqlx::query_scalar::<_, i64>(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) \
             SELECT NULL, $1, $2, $3 \
              WHERE u_notice_enabled($1, 'class_demote') \
                AND NOT EXISTS (SELECT 1 FROM spark_ledger \
                                WHERE idempotency_key = $4) \
             RETURNING 1",
        )
        .bind(uid)
        .bind("等级调整通知")
        .bind(format!(
            "很抱歉，{username}：因近期上传量/做种时长未达到当前等级要求，\
             你的等级已调整为「{}」。持续做种与分享即可自动恢复升级，\
             升级条件见等级页；如有疑问请回复本信。",
            name.unwrap_or_default()
        ))
        .bind(format!("class_demote_notice:{uid}:{new_class}"))
        .fetch_optional(db)
        .await
        .unwrap_or(None)
        .unwrap_or(0);
        if sent > 0 {
            // 预告锚落 spark_ledger 纯幂等标记（amount=0 不动账，kind 区分
            // 于一切经济流；idempotency_key 即防重发闸）
            let _ = sqlx::query(
                "INSERT INTO spark_ledger \
                    (id, user_id, amount, kind, idempotency_key) \
                 SELECT nextval('spark_ledger_id_seq'), $1, 0, \
                     'notice_marker', $2 \
                  WHERE NOT EXISTS (SELECT 1 FROM spark_ledger \
                                    WHERE idempotency_key = $2)",
            )
            .bind(uid)
            .bind(format!("class_demote_notice:{uid}:{new_class}"))
            .execute(db)
            .await;
        }
    }
    if !demoted.is_empty() {
        tracing::info!(n = demoted.len(), "users demoted");
    }
    Ok(())
}
