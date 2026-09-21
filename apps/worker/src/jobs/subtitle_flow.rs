//! 字幕工作流 worker 侧（0148）：超时扫描 + 月度评选候选。
//! SQL 与 api 侧 content_http/subtitles_{sweep,awards}.rs 同源（worker 是
//! 独立 crate，不依赖 api 的模块树）。

use sqlx::PgPool;

/// 幂等发火花（事务内流水+余额同护栏；与 achievement_grant 同口径）
async fn spark_award(
    db: &PgPool,
    uid: i64,
    amount: i64,
    kind: &str,
    idem: &str,
) {
    let mut tx = match db.begin().await {
        Ok(t) => t,
        Err(_) => return,
    };
    let ok1 = sqlx::query(
        "INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key) \
         SELECT nextval('spark_ledger_id_seq'), $1, $2, $3, $4 \
         WHERE NOT EXISTS (SELECT 1 FROM spark_ledger \
         WHERE idempotency_key = $4)",
    )
    .bind(uid)
    .bind(amount)
    .bind(kind)
    .bind(idem)
    .execute(&mut *tx)
    .await
    .map(|r| r.rows_affected() > 0)
    .unwrap_or(false);
    let ok2 = sqlx::query(
        "UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1 \
         AND NOT EXISTS (SELECT 1 FROM spark_ledger \
         WHERE idempotency_key = $3)",
    )
    .bind(uid)
    .bind(amount)
    .bind(idem)
    .execute(&mut *tx)
    .await
    .map(|r| r.rows_affected() > 0)
    .unwrap_or(false);
    if ok1 && ok2 {
        let _ = tx.commit().await;
    } else {
        let _ = tx.rollback().await;
    }
}

/// 认领超时回池（3→0）+ 交稿超时自动验收（4→1）
pub(crate) async fn subreq_sweep(db: &PgPool) -> anyhow::Result<()> {
    // 认领超时：deadline 已过 → 回池 + 通知认领人
    let expired: Vec<(i64, Option<i64>)> = sqlx::query_as(
        "UPDATE subtitle_requests SET status = 0, claimed_by = NULL, \
         claimed_at = NULL, deadline_at = NULL, crew = '[]' \
         WHERE status = 3 AND deadline_at IS NOT NULL AND deadline_at < now() \
         RETURNING id, claimed_by",
    )
    .fetch_all(db)
    .await?;
    for (rid, claimer) in &expired {
        if let Some(uid) = claimer {
            let _ = sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) \
                 VALUES (NULL, $1, '字幕认领已超时', $2)",
            )
            .bind(uid)
            .bind(format!(
                "你认领的求字幕 #{rid} 已超时回池，其他用户可重新认领。"
            ))
            .execute(db)
            .await;
        }
    }
    // 交稿超时：自动验收（天数可配，默认 3；1-30 钳位）。
    // 分账与 api 侧 subtitles_close::settle_crew 同键域
    // （subtitle-bounty-pay:{rid}:{uid}，幂等键互斥）——即使人工验收与
    // 自动验收并发也不会双发（F4 修复：此前 auto 用独立键整池给主认领人）。
    let days: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = \
         'subtitle_accept_timeout_days'",
    )
    .fetch_optional(db)
    .await?
    .flatten()
    .unwrap_or_else(|| "3".into());
    let days: i64 = days.trim().parse().unwrap_or(3).clamp(1, 30);
    let auto: Vec<(i64, i64, i64, i64, serde_json::Value)> = sqlx::query_as(
        "UPDATE subtitle_requests SET status = 1, paid_at = now() WHERE \
         status = 4 AND deliver_at < now() - ($1 || ' days')::interval \
         RETURNING id, bounty, COALESCE(claimed_by, user_id), user_id, crew",
    )
    .bind(days.to_string())
    .fetch_all(db)
    .await?;
    let auto_n = auto.len();
    for (rid, bounty, claimer, owner, crew) in auto {
        let mut remainder = bounty;
        if let Some(list) = crew.as_array() {
            for m in list.iter().filter(|m| {
                m.get("accepted").and_then(|v| v.as_bool())
                    .unwrap_or(false)
            }) {
                let (Some(uid), Some(share)) = (
                    m.get("user_id").and_then(|v| v.as_i64()),
                    m.get("share").and_then(|v| v.as_i64()),
                ) else {
                    continue;
                };
                let amount = bounty * share / 100;
                if amount <= 0 || uid == claimer {
                    continue;
                }
                let idem = format!("subtitle-bounty-pay:{}:{}", rid, uid);
                spark_award(db, uid, amount, "subtitle_bounty", &idem).await;
                remainder -= amount;
            }
        }
        if remainder > 0 {
            let idem = format!("subtitle-bounty-pay:{}:{}", rid, claimer);
            spark_award(db, claimer, remainder, "subtitle_bounty", &idem)
                .await;
        }
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) \
             VALUES (NULL, $1, '字幕已自动验收', $2)",
        )
        .bind(owner)
        .bind(format!(
            "求字幕 #{rid} 验收超时已自动结算（赏金 {bounty} 已按份额分发）。"
        ))
        .execute(db)
        .await;
    }
    if !expired.is_empty() || auto_n > 0 {
        tracing::info!(
            released = expired.len(),
            auto_accepted = auto_n,
            "subtitle request timeouts swept"
        );
    }
    Ok(())
}

/// 月度评选候选（月初生成上月；subtitle_award=0 时跳过）
pub(crate) async fn subawards_build(db: &PgPool) -> anyhow::Result<()> {
    let on: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'subtitle_award'",
    )
    .fetch_optional(db)
    .await?
    .flatten();
    if on.as_deref() != Some("1") {
        return Ok(());
    }
    // 上个自然月
    let now = chrono::Utc::now();
    let first = format!("{}-01", now.format("%Y-%m"));
    let prev = chrono::NaiveDate::parse_from_str(&first, "%Y-%m-%d")
        .map(|d| d.pred_opt().unwrap_or(d))
        .unwrap_or(now.date_naive());
    let period = prev.format("%Y-%m").to_string();
    let mut n = 0u64;
    for tier in ["human", "ai"] {
        let res = sqlx::query(
            r#"
            INSERT INTO subtitle_awards
                (period, subtitle_id, user_id, score, rank, tier)
            SELECT $1, s.id, s.user_id,
                   (s.rating_sum::float8 / NULLIF(s.rating_count, 0)
                    * ln(1 + s.rating_count)
                    + least(s.downloads, 500) * 0.05
                    + CASE WHEN s.verified THEN 3 ELSE 0 END)::numeric(10,2),
                   0, $2
            FROM subtitles s
            WHERE s.deleted_at IS NULL AND s.status = 1
              AND s.rating_count > 0
              AND s.created_at >= ($1 || '-01')::date
              AND s.created_at <  ($1 || '-01')::date + interval '1 month'
              AND ($2::text = 'human'
                   AND (NOT s.machine_translated
                        OR COALESCE(s.proofreader, '') <> '')
                   OR $2::text = 'ai'
                   AND s.machine_translated
                   AND COALESCE(s.proofreader, '') = '')
            ORDER BY 3 DESC LIMIT 10
            ON CONFLICT (period, subtitle_id) DO NOTHING
            "#,
        )
        .bind(&period)
        .bind(tier)
        .execute(db)
        .await?;
        n += res.rows_affected();
    }
    if n > 0 {
        tracing::info!(
            period = %period, added = n,
            "subtitle award candidates built"
        );
    }
    Ok(())
}
