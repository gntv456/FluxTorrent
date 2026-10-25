//! 字幕工作流 worker 侧（0148）：超时扫描 + 月度评选候选。
//! SQL 与 api 侧 content_http/subtitles_{sweep,awards}.rs 同源（worker 是
//! 独立 crate，不依赖 api 的模块树）。

use sqlx::PgPool;

/// 幂等发火花（**事务内**版，P-416）：调用方的 CAS 与这里必须同一个事务。
///
/// 旧版收 `&PgPool` 自己开事务，并把一切失败折成「warn + 跳过」；上层 CAS
/// `4→1` 却是 autocommit 先落库的 ⇒ 「一分钱没发出去」这件事永远不会被重试，
/// 而站内信照发「赏金已按份额分发」（P-412 同族，这次在 worker 侧）。
/// 现在失败一律 `?` 上抛 ⇒ 上层回滚 ⇒ 单子留在 status=4，下一 tick 重试。
///
/// 幂等护栏只在本事务 INSERT 上判断：事务内已插入的行对本事务可见，若 UPDATE
/// 仍带同款 NOT EXISTS 会恒 0 行 → 整体回滚 → 支付静默丢失（链路缺陷 #10）。
/// 返回 `true`=本次真出账；`false`=同键已在册（钱早就发出去了，只补状态位）。
async fn spark_award_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    amount: i64,
    kind: &str,
    idem: &str,
) -> anyhow::Result<bool> {
    let inserted = sqlx::query(
        "INSERT INTO spark_ledger \
         (id, user_id, amount, kind, idempotency_key, balance_after) \
         SELECT nextval('spark_ledger_id_seq'), $1, $2, $3, $4,\
           u.spark_balance + $2 FROM users u WHERE u.id = $1 \
         AND NOT EXISTS (SELECT 1 FROM spark_ledger \
         WHERE idempotency_key = $4)",
    )
    .bind(uid)
    .bind(amount)
    .bind(kind)
    .bind(idem)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if inserted == 0 {
        // 同键已在册而单子仍是 status=4 ⇒ 上一次「发了钱没落状态」的撕裂残留：
        // 这里只补状态，绝不双发
        tracing::warn!(uid, idem, "字幕赏金幂等键已在册：只补状态不重复出账");
        return Ok(false);
    }
    let balanced = sqlx::query(
        "UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1",
    )
    .bind(uid)
    .bind(amount)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if balanced == 0 {
        // 账号不存在（并发删号）：上抛让整笔回滚，别把「已结算」落在没人的单上
        anyhow::bail!("字幕赏金入账失败：账号 {uid} 不存在（{idem}）");
    }
    Ok(true)
}

/// 认领超时回池（3→0）+ 交稿超时自动验收（4→1）
pub(crate) async fn subreq_sweep(db: &PgPool) -> anyhow::Result<()> {
    // 认领超时：deadline 已过 → 回池 + 通知认领人
    //
    // P-49（本批实测，闸门 AR 抓到）：`UPDATE ... SET claimed_by = NULL
    // ... RETURNING claimed_by` 里 RETURNING 给的是**更新后**的行，
    // 取回的恒是 NULL ⇒ 下面的 `if let Some(uid)` 永远不成立，
    // 「超时=系统代行弃单」的 subreq.abandon 审计与认领人站内信双双静默
    // 丢失（实测：回池成功、audit_log 里 subreq.abandon 全库 0 条）。
    // 后果不只是少一封信：F3 的弃单禁令（30 天内 3 次禁认领）正是靠这条
    // 计数补上「主动弃单」之外的漏洞，漏计等于故意拖到超时的霸单人免罚。
    // 正确写法=先用 CTE 快照候选行，RETURNING 取 cand 的旧值。
    let expired: Vec<(i64, Option<i64>)> = sqlx::query_as(
        "SELECT id, claimed_by FROM subtitle_requests WHERE status = 3 \
         AND deadline_at IS NOT NULL AND deadline_at < now()",
    )
    .fetch_all(db)
    .await?;
    let mut released = 0usize;
    for (rid, claimer) in &expired {
        // P-416：回池 CAS、subreq.abandon 审计、认领人站内信同一事务。
        // P-49 修的是「RETURNING 取到更新后的行」，但三步各自 autocommit +
        // 两处 `let _ =` 仍然让失败无声——漏一条审计就等于漏计一次弃单，
        // 「30 天内 3 次禁认领」的门槛靠拖超时就能绕过。现在任一写失败
        // ⇒ 整单回滚，留在 status=3，下一 tick 重试。
        let mut tx = db.begin().await?;
        let n = sqlx::query(
            "UPDATE subtitle_requests SET status = 0, claimed_by = NULL, \
             claimed_at = NULL, deadline_at = NULL, crew = '[]' \
             WHERE id = $1 AND status = 3",
        )
        .bind(rid)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if n == 0 {
            // 这一轮里已被人手动弃单/重新认领：不用回池，事务随 drop 回滚
            continue;
        }
        if let Some(uid) = claimer {
            sqlx::query(
                "INSERT INTO audit_log (id, actor_id, action, ref) \
                 VALUES (nextval('audit_log_id_seq'), $1, 'subreq.abandon', \
                 $2::jsonb)",
            )
            .bind(uid)
            .bind(format!("{{\"by\":\"sweep-timeout\",\"id\":{rid}}}"))
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) \
                 VALUES (NULL, $1, '字幕认领已超时', $2)",
            )
            .bind(uid)
            .bind(format!(
                "你认领的求字幕 #{rid} 已超时回池（计入弃单次数），其他用户可重新认领。"
            ))
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        released += 1;
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
    // 候选只读出来，不在这里翻状态：CAS 与分账必须在同一个事务里（P-416）。
    // 旧写法 `UPDATE ... SET status = 1 ... WHERE status = 4 RETURNING` 先
    // autocommit 把单子写成「已自动验收」，随后逐笔 spark_award 把一切失败
    // 折成「warn + 跳过」——于是发放失败永远不会被重试，站内信还写着「赏金
    // 已按份额分发」。同键域互斥（F4）挡住了双发，挡不住零发。
    let auto: Vec<(i64, i64, i64, i64, serde_json::Value)> = sqlx::query_as(
        "SELECT id, bounty, COALESCE(claimed_by, user_id), user_id, crew \
         FROM subtitle_requests WHERE status = 4 AND deliver_at IS NOT NULL \
         AND deliver_at < now() - ($1 || ' days')::interval",
    )
    .bind(days.to_string())
    .fetch_all(db)
    .await?;
    let mut auto_ok = 0usize;
    for (rid, bounty, claimer, owner, crew) in auto {
        let mut tx = db.begin().await?;
        let n = sqlx::query(
            "UPDATE subtitle_requests SET status = 1, paid_at = now() \
             WHERE id = $1 AND status = 4",
        )
        .bind(rid)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if n == 0 {
            // 人工验收抢先了：本单不用自动结算，事务随 drop 回滚
            continue;
        }
        let mut remainder = bounty;
        if let Some(list) = crew.as_array() {
            for m in list.iter().filter(|m| {
                m.get("accepted").and_then(|v| v.as_bool()).unwrap_or(false)
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
                spark_award_tx(&mut tx, uid, amount, "subtitle_bounty", &idem)
                    .await?;
                remainder -= amount;
            }
        }
        if remainder > 0 {
            let idem = format!("subtitle-bounty-pay:{}:{}", rid, claimer);
            spark_award_tx(
                &mut tx,
                claimer,
                remainder,
                "subtitle_bounty",
                &idem,
            )
            .await?;
        }
        // 通知与结算同事务：钱没落地就别发「已分发」这句话
        sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) \
             VALUES (NULL, $1, '字幕已自动验收', $2)",
        )
        .bind(owner)
        .bind(format!(
            "求字幕 #{rid} 验收超时已自动结算（赏金 {bounty} 已按份额分发）。"
        ))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        auto_ok += 1;
    }
    if released > 0 || auto_ok > 0 {
        tracing::info!(
            released,
            auto_accepted = auto_ok,
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
            ORDER BY 4 DESC LIMIT 10
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
