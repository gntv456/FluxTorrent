//! 绩效结算逐行结算循环（0106）。
//! 从 jobs.rs/jixiao.rs 按域拆出——登记行的事务推进与发薪都在此。

use sqlx::PgPool;

/// 逐行结算上一期 admin 登记行：状态推进与发薪同事务（崩溃整体回滚）。
pub(crate) async fn jixiao_settle_loop(
    db: &PgPool,
    pending: Vec<PendingClaim>,
    prev: &str,
    site_step: i64,
    site_pct: i64,
) -> anyhow::Result<u64> {
    let mut done: u64 = 0;
    for p in &pending {
        let mut tx = db.begin().await?;

        // 用户当前指标（期末值；基线 = 登记行 base_*，期初快照表缺它兜底——
        // 与 API compute_metrics 同优先级：快照表为主，这里登记行在期内必然存在，
        // 且结算时快照表写的是「下期」，本期基线只能来自登记行/上期快照）
        let cur_vals: Option<(i64, i64, i64)> = sqlx::query_as(
            "SELECT u.uploaded, u.uploaded, \
                    (SELECT count(*) FROM torrents tr WHERE tr.owner_id = u.id AND tr.approval_status = 1) \
             FROM users u WHERE u.id = $1",
        )
        .bind(p.user_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((uploaded_now, _dup, uploads_now)) = cur_vals else {
            // 用户已删：登记行作废不结算（不扣不发，keep 事实行）
            sqlx::query("UPDATE jixiao_claims SET status = 3, settled_at = now() WHERE id = $1")
                .bind(p.id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            continue;
        };
        let seed_seconds_now: i64 = sqlx::query_scalar(
            "SELECT COALESCE(sum(seeded_seconds),0)::bigint FROM snatches WHERE user_id = $1",
        )
        .bind(p.user_id)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);
        let seed_hours =
            ((seed_seconds_now - p.base_seed_seconds).max(0)) / 3600;
        let uploads_delta = (uploads_now - p.base_uploads).max(0);
        let uploaded_delta = (uploaded_now - p.base_uploaded).max(0);
        let seed_days: i64 = sqlx::query_scalar(
            "SELECT count(DISTINCT date_trunc('day', s.last_seen_at)) FROM snatches s \
             WHERE s.user_id = $1 AND to_char(s.last_seen_at, 'YYYY-MM') = $2 AND s.seeding",
        )
        .bind(p.user_id)
        .bind(&prev)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);
        let seeding_count: i64 = sqlx::query_scalar(
            "SELECT count(DISTINCT torrent_id) FROM snatches WHERE user_id = $1 AND seeding",
        )
        .bind(p.user_id)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);
        let seed_size: i64 = sqlx::query_scalar(
            "SELECT COALESCE(sum(t.size),0)::bigint FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.seeding",
        )
        .bind(p.user_id)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);
        let (up_month, down_month): (i64, i64) = sqlx::query_as(
            "SELECT COALESCE(sum(delta_up),0)::bigint, COALESCE(sum(delta_down),0)::bigint \
             FROM traffic_ledger WHERE user_id = $1 AND to_char(window_start, 'YYYY-MM') = $2",
        )
        .bind(p.user_id)
        .bind(&prev)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or((0, 0));
        let ops: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM audit_log WHERE actor_id = $1 AND to_char(created_at, 'YYYY-MM') = $2",
        )
        .bind(p.user_id)
        .bind(&prev)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);

        // 与 API compute_metrics 同键集合（结算快照留全量，判定只读 min_requirements）
        let metrics = serde_json::json!({
            "uploaded": up_month.max(uploaded_delta), "downloaded": down_month, "uploads": uploads_delta,
            "seeding_count": seeding_count, "seed_size": seed_size,
            "seed_size_tb": seed_size / 1_099_511_627_776,
            "seed_hours": seed_hours, "seed_days": seed_days, "ops": ops,
        });

        // 达标判定：min_requirements 全部满足（与 /jixiao/claim 同口径）
        let mut all_ok = true;
        if let Some(reqs) = p.min_requirements.as_object() {
            for (k, v) in reqs {
                let required = v.as_i64().unwrap_or(0);
                if required > 0
                    && metrics.get(k).and_then(|x| x.as_i64()).unwrap_or(0)
                        < required
                {
                    all_ok = false;
                    break;
                }
            }
        }

        if all_ok {
            // 达标月数（含本期）：历史 status=1 行数 + 1
            let months_before: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM jixiao_claims WHERE user_id = $1 AND type_id = $2 AND status = 1",
            )
            .bind(p.user_id)
            .bind(p.type_id)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(0);
            // 岗位级加成优先（jixiao_types.bonus_rules：admin 表单「每 N 月 +M%」），
            // 未配置/非法回落全站 site_settings——与 API 侧 jixiao_bonus 同口径
            let (step, pct) = match p.bonus_rules.as_object().map(|r| {
                (
                    r.get("months_per_step").and_then(|v| v.as_i64()),
                    r.get("percent_per_step").and_then(|v| v.as_i64()),
                )
            }) {
                Some((Some(s), Some(pc))) if s > 0 && pc >= 0 => (s, pc),
                _ => (site_step, site_pct),
            };
            let bonus = p.base_pay * pct / 100 * ((months_before + 1) / step);
            let total = p.base_pay + bonus;

            // CAS：与用户补领互斥（先到先得）
            let upd = sqlx::query(
                "UPDATE jixiao_claims SET status = 1, settled_at = now(), amount = $2, bonus_paid = $3, \
                        metrics_at_settle = $4, \
                        metrics_snapshot = metrics_snapshot || '{\"settle_by\":\"worker\"}'::jsonb \
                 WHERE id = $1 AND settled_at IS NULL AND status = 0",
            )
            .bind(p.id)
            .bind(total)
            .bind(bonus)
            .bind(&metrics)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if upd == 0 {
                tx.rollback().await?;
                continue;
            }
            if total > 0 {
                let idem = format!("jixiao:settle:{}", p.id);
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
                )
                .bind(&idem)
                .fetch_one(&mut *tx)
                .await?;
                if !exists {
                    let balance: i64 = sqlx::query_scalar(
                        "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
                    )
                    .bind(p.user_id)
                    .fetch_one(&mut *tx)
                    .await?;
                    sqlx::query(
                        "INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key, balance_after) \
                         VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'jixiao_reward', $3, $4)",
                    )
                    .bind(p.user_id)
                    .bind(total)
                    .bind(&idem)
                    .bind(balance + total)
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query(
                        "UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1",
                    )
                    .bind(p.user_id)
                    .bind(total)
                    .execute(&mut *tx)
                    .await?;
                }
            }
            let _ = sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
            )
            .bind(p.user_id)
            .bind("绩效考核工资已发放")
            .bind(format!(
                "你本期（{}）的「{}」考核已达标，工资 {} 魔力（含连续达标加成 {}）已自动发放到账。",
                prev, p.name, total, bonus
            ))
            .execute(&mut *tx)
            .await;
        } else {
            let upd = sqlx::query(
                "UPDATE jixiao_claims SET status = 2, settled_at = now(), metrics_at_settle = $2, \
                        metrics_snapshot = metrics_snapshot || '{\"settle_by\":\"worker\"}'::jsonb \
                 WHERE id = $1 AND settled_at IS NULL AND status = 0",
            )
            .bind(p.id)
            .bind(&metrics)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if upd == 0 {
                tx.rollback().await?;
                continue;
            }
            // 平实文案（不羞辱纪律：不指责，只陈述事实 + 收益不受影响的说明）
            let _ = sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
            )
            .bind(p.user_id)
            .bind("绩效考核本期未达标")
            .bind(format!(
                "你本期（{}）的「{}」考核未达到岗位指标，本期工资未发放。\
                 你已产生的做种/上传收益不受影响，下期继续。",
                prev, p.name
            ))
            .execute(&mut *tx)
            .await;
        }

        tx.commit().await?;
        done += 1;
    }

    Ok(done)
}

#[derive(sqlx::FromRow)]
pub(crate) struct PendingClaim {
    id: i64,
    user_id: i64,
    type_id: i64,
    name: String,
    base_pay: i64,
    min_requirements: serde_json::Value,
    /// 岗位级加成配置（0106：admin 表单写入；缺省回落全站配置）
    #[sqlx(default)]
    bonus_rules: serde_json::Value,
    base_seed_seconds: i64,
    base_uploaded: i64,
    base_uploads: i64,
}
