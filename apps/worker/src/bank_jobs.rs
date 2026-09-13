//! 银行每日结算（0048 火花银行对齐）：活期复利结息、贷款计息、逾期罚息与
//! 自动扣款、定期到期自动结清、站内信通知。由 worker 日循环调度一次。

use sqlx::PgPool;

/// 活期结息：利息滚入活期本金（复利）+ 利息明细。
/// last_interest_date < 今天 的账户按天数补结；幂等靠日期游标。
/// 利息为 0 的账户也要推进游标（否则天数会在游标里累积后一次补结，口径变成"按存入日复利"）。
pub async fn bank_demand_settle(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        WITH due AS (
            SELECT a.id, a.user_id, a.balance, a.daily_rate_bp,
                   (CURRENT_DATE - COALESCE(a.last_interest_date, CURRENT_DATE)) AS days
            FROM bank_demand_accounts a
            WHERE a.balance > 0
              AND a.last_interest_date < CURRENT_DATE
        ),
        calc AS (
            SELECT id, user_id, daily_rate_bp,
                   (balance * daily_rate_bp * days / 10000)::bigint AS interest
            FROM due
        ),
        upd AS (
            UPDATE bank_demand_accounts a
            SET balance = a.balance + c.interest,
                last_interest_date = CURRENT_DATE,
                updated_at = now()
            FROM calc c
            WHERE a.id = c.id
            RETURNING a.id
        )
        INSERT INTO bank_interest_records (user_id, kind, reference_id, amount, rate_bp)
        SELECT c.user_id, 'demand', c.id, c.interest, c.daily_rate_bp
        FROM calc c JOIN upd ON upd.id = c.id AND c.interest > 0
        "#,
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// 定期每日结息（settle_mode='daily'）：按日利率把利息发到用户余额，到期只还本。
/// 日利率 = 年化 rate / 365，整数向下取整；幂等靠 last_interest_date 游标 + ledger 幂等键。
pub async fn bank_fixed_daily_settle(db: &PgPool) -> anyhow::Result<u64> {
    // rate 是 NUMERIC，sqlx 不直接解 NUMERIC→f64，取 text 自行解析
    let rows: Vec<(i64, i64, i64, String, chrono::NaiveDate)> = sqlx::query_as(
        "SELECT id, user_id, amount, rate::text, COALESCE(last_interest_date, (start_at::date - 1)) \
         FROM bank_deposits \
         WHERE status = 0 AND settle_mode = 'daily' \
           AND COALESCE(last_interest_date, (start_at::date - 1)) < LEAST(CURRENT_DATE, maturity_at::date) \
         LIMIT 500",
    )
    .fetch_all(db)
    .await?;
    let mut count = 0u64;
    for (id, user_id, amount, rate_text, last_date) in rows {
        let annual_rate: f64 = rate_text.parse().unwrap_or(0.0);
        let end = chrono::Utc::now().date_naive();
        let days = (end - last_date).num_days().max(0);
        if days == 0 {
            continue;
        }
        // 年化 → 日息，向下取整防超发；与 maturity_interest 全期口径一致
        let interest = ((amount as f64) * annual_rate * (days as f64) / 365.0).floor() as i64;
        let mut tx = db.begin().await?;
        let updated = sqlx::query(
            "UPDATE bank_deposits SET paid_interest = paid_interest + $2, last_interest_date = $3 \
             WHERE id = $1 AND status = 0 AND COALESCE(last_interest_date, (start_at::date - 1)) = $4",
        )
        .bind(id)
        .bind(interest)
        .bind(end)
        .bind(last_date)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            tx.rollback().await?;
            continue;
        }
        if interest > 0 {
            let idem = format!("fixed_daily:{}:{}", id, end.format("%Y%m%d"));
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
            )
            .bind(&idem)
            .fetch_one(&mut *tx)
            .await?;
            if !exists {
                let balance: i64 =
                    sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE")
                        .bind(user_id)
                        .fetch_one(&mut *tx)
                        .await?;
                sqlx::query(
                    "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \
                     VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'bank_fixed_interest', 'bank', $3, $4, $5)",
                )
                .bind(user_id)
                .bind(interest)
                .bind(id)
                .bind(&idem)
                .bind(balance + interest)
                .execute(&mut *tx)
                .await?;
                sqlx::query("UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1")
                    .bind(user_id)
                    .bind(interest)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query(
                    "INSERT INTO bank_interest_records (user_id, kind, reference_id, amount, rate_bp) \
                     VALUES ($1, 'fixed', $2, $3, 0)",
                )
                .bind(user_id)
                .bind(id)
                .bind(interest)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
        count += 1;
    }
    Ok(count)
}

/// 贷款正常期计息：利息只计提不并本金（结清时一次收），记明细。
/// 计息截止 min(今天, 到期日)；当日入账口径（首日计 1 天）。
pub async fn bank_loan_interest(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        WITH due AS (
            SELECT l.id AS loan_id, l.user_id, l.remaining, l.daily_rate_bp, l.last_interest_date,
                   l.created_at,
                   LEAST(CURRENT_DATE, l.due_at::date) AS end_date
            FROM bank_loans l
            WHERE l.status = 'active'
              AND COALESCE(l.last_interest_date, (l.created_at::date - 1)) < LEAST(CURRENT_DATE, l.due_at::date)
        ),
        calc AS (
            SELECT d.loan_id, d.user_id, d.daily_rate_bp,
                   -- 向上取整防逃息（economy::loan_interest 同口径：(x + 9999) / 10000）
                   (d.remaining * d.daily_rate_bp *
                     (d.end_date - COALESCE(d.last_interest_date, (d.created_at::date - 1))) + 9999) / 10000
                     AS interest
            FROM due d
        ),
        upd AS (
            UPDATE bank_loans l
            SET accrued_interest = l.accrued_interest + c.interest,
                last_interest_date = LEAST(CURRENT_DATE, l.due_at::date)
            FROM calc c
            WHERE l.id = c.loan_id AND c.interest > 0
            RETURNING l.id
        )
        INSERT INTO bank_interest_records (user_id, kind, reference_id, amount, rate_bp)
        SELECT c.user_id, 'loan', c.loan_id, -c.interest, c.daily_rate_bp
        FROM calc c JOIN upd ON upd.id = c.loan_id
        "#,
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// 逾期罚息：逾期后每日按罚息率计息并并入本金（与火花口径一致）。
/// 逾期超 term_days 一半仍未结清的贷款翻转为 defaulted（schema 声明的状态；
/// 修复前永远停在 active，API 侧无法区分正常/逾期贷款）。
pub async fn bank_overdue_penalty(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        WITH due AS (
            SELECT id, user_id, remaining, penalty_rate_bp,
                   (CURRENT_DATE - COALESCE(last_interest_date, CURRENT_DATE - 1)) AS days
            FROM bank_loans
            WHERE status = 'active' AND due_at < now()
              AND last_interest_date < CURRENT_DATE
        ),
        calc AS (
            SELECT id, user_id, penalty_rate_bp,
                   -- 罚息并入本金（欠款方向），与贷款计息同口径向上取整
                   (remaining * penalty_rate_bp * days + 9999) / 10000 AS penalty
            FROM due
        ),
        upd AS (
            UPDATE bank_loans l
            SET remaining = l.remaining + c.penalty,
                last_interest_date = CURRENT_DATE
            FROM calc c
            WHERE l.id = c.id AND c.penalty > 0
            RETURNING l.id
        ),
        ins AS (
            INSERT INTO bank_interest_records (user_id, kind, reference_id, amount, rate_bp)
            SELECT c.user_id, 'loan_penalty', c.id, -c.penalty, c.penalty_rate_bp
            FROM calc c JOIN upd ON upd.id = c.id
            RETURNING reference_id
        )
        UPDATE bank_loans l
        SET status = 'defaulted'
        WHERE l.status = 'active'
          AND l.due_at < now() - make_interval(days => l.term_days / 2)
        "#,
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// 严重逾期自动扣款：逾期超 N 天，先扣活期再扣站内余额，扣清为止。
/// 负余额口径走站点设置 bank_allow_negative；每次调度最多扣一次（游标 = updated_at 当日已处理）。
pub async fn bank_auto_deduct(
    db: &PgPool,
    deduct_days: i32,
    allow_negative: bool,
) -> anyhow::Result<u64> {
    let loans: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT id, user_id, remaining FROM bank_loans \
         WHERE status = 'active' AND due_at < now() - ($1 || ' days')::interval \
           AND (last_interest_date IS NULL OR last_interest_date <= CURRENT_DATE)",
    )
    .bind(deduct_days.to_string())
    .fetch_all(db)
    .await?;

    let mut count = 0u64;
    for (loan_id, user_id, remaining) in loans {
        let mut tx = db.begin().await?;
        // 先扣活期：RETURNING 的是更新后的余额，实扣额 = 原余额 - 新余额（修复扣全款仍报 0 的通账 bug）
        let before_after: Option<(i64, i64)> = sqlx::query_as(
            "UPDATE bank_demand_accounts SET balance = balance - LEAST(balance, $2), updated_at = now() \
             WHERE user_id = $1 RETURNING (balance + LEAST(balance, $2)) AS before, balance",
        )
        .bind(user_id)
        .bind(remaining)
        .fetch_optional(&mut *tx)
        .await?;
        let from_demand = before_after
            .map(|(before, after)| before - after)
            .unwrap_or(0);
        let left = remaining - from_demand;
        let mut deducted = from_demand;
        if left > 0 {
            if allow_negative {
                // 负余额口径：直接扣清
                sqlx::query(
                    "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key) \
                     SELECT nextval('spark_ledger_id_seq'), $2, -$3, 'bank_auto_deduct', 'bank', $1, \
                            'auto_deduct:' || $1 || ':' || to_char(now(), 'YYYYMMDD') \
                     WHERE NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = 'auto_deduct:' || $1 || ':' || to_char(now(), 'YYYYMMDD'))",
                )
                .bind(loan_id)
                .bind(user_id)
                .bind(left)
                .execute(&mut *tx)
                .await?;
                sqlx::query("UPDATE users SET spark_balance = spark_balance - $2 WHERE id = $1")
                    .bind(user_id)
                    .bind(left)
                    .execute(&mut *tx)
                    .await?;
                deducted += left;
            } else {
                // 仅扣现有余额（balance_after 快照行锁口径）
                let balance: i64 =
                    sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE")
                        .bind(user_id)
                        .fetch_one(&mut *tx)
                        .await?;
                let take = balance.min(left).max(0);
                if take > 0 {
                    let idem = format!(
                        "auto_deduct:{}:{}",
                        loan_id,
                        chrono::Utc::now().format("%Y%m%d")
                    );
                    let exists: bool = sqlx::query_scalar(
                        "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
                    )
                    .bind(&idem)
                    .fetch_one(&mut *tx)
                    .await?;
                    if !exists {
                        sqlx::query(
                            "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \
                             VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'bank_auto_deduct', 'bank', $3, $4, $5)",
                        )
                        .bind(user_id)
                        .bind(-take)
                        .bind(loan_id)
                        .bind(&idem)
                        .bind(balance - take)
                        .execute(&mut *tx)
                        .await?;
                        sqlx::query(
                            "UPDATE users SET spark_balance = spark_balance - $2 WHERE id = $1",
                        )
                        .bind(user_id)
                        .bind(take)
                        .execute(&mut *tx)
                        .await?;
                        deducted += take;
                    }
                }
            }
        }
        if deducted > 0 {
            let new_remaining = remaining - deducted;
            if new_remaining <= 0 {
                sqlx::query(
                    "UPDATE bank_loans SET remaining = 0, accrued_interest = 0, status = 'paid', paid_at = now() \
                     WHERE id = $1",
                )
                .bind(loan_id)
                .execute(&mut *tx)
                .await?;
            } else {
                sqlx::query("UPDATE bank_loans SET remaining = $2 WHERE id = $1")
                    .bind(loan_id)
                    .bind(new_remaining)
                    .execute(&mut *tx)
                    .await?;
            }
            // 扣款站内信
            sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
            )
            .bind(user_id)
            .bind("银行逾期贷款自动扣款通知")
            .bind(format!(
                "您的贷款已逾期，系统自动扣款 {deducted} 火花（活期 {from_demand}）。剩余欠款 {}。请尽快结清，逾期期间每日加计罚息。",
                (remaining - deducted).max(0)
            ))
            .execute(&mut *tx)
            .await?;
            count += 1;
        }
        tx.commit().await?;
    }
    Ok(count)
}

/// 定期到期自动结清（幂等键 = withdraw:{deposit_id}，与手动支取同键防双发）。
/// daily 模式：利息已按日发过，到期只还本；maturity 模式：本息一次付清。
pub async fn bank_fixed_mature(db: &PgPool) -> anyhow::Result<u64> {
    let rows: Vec<(i64, i64, i64, i64, i64, String)> = sqlx::query_as(
        "SELECT id, user_id, amount, interest, paid_interest, settle_mode FROM bank_deposits \
         WHERE status = 0 AND maturity_at <= now() LIMIT 500",
    )
    .fetch_all(db)
    .await?;
    let mut count = 0u64;
    for (id, user_id, amount, interest, paid_interest, mode) in rows {
        let daily = mode == "daily";
        let final_interest = if daily { 0 } else { interest };
        let mut tx = db.begin().await?;
        let updated = sqlx::query(
            "UPDATE bank_deposits SET status = 1, settled_at = now() WHERE id = $1 AND status = 0",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            tx.rollback().await?;
            continue;
        }
        let idem = format!("withdraw:{}", id);
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
        )
        .bind(&idem)
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            let balance: i64 =
                sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE")
                    .bind(user_id)
                    .fetch_one(&mut *tx)
                    .await?;
            let payable = amount + final_interest;
            sqlx::query(
                "INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key, balance_after) \
                 VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'bank_withdraw', $3, $4)",
            )
            .bind(user_id)
            .bind(payable)
            .bind(&idem)
            .bind(balance + payable)
            .execute(&mut *tx)
            .await?;
            sqlx::query("UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1")
                .bind(user_id)
                .bind(payable)
                .execute(&mut *tx)
                .await?;
            if final_interest > 0 {
                sqlx::query(
                    "INSERT INTO bank_interest_records (user_id, kind, reference_id, amount, rate_bp) \
                     VALUES ($1, 'fixed', $2, $3, 0)",
                )
                .bind(user_id)
                .bind(id)
                .bind(final_interest)
                .execute(&mut *tx)
                .await?;
            }
            sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
            )
            .bind(user_id)
            .bind("银行存款到期通知")
            .bind(if daily {
                format!(
                    "您的定期存款已到期，本金 {amount} 火花已返还；存期内已按日发放利息共 {paid_interest} 火花。"
                )
            } else {
                format!(
                    "您的定期存款已到期，本金 {amount} 火花与利息 {interest} 火花已自动返还到您的账户。"
                )
            })
            .execute(&mut *tx)
            .await?;
            count += 1;
        }
        tx.commit().await?;
    }
    Ok(count)
}

/// 到期前提醒：贷款 3 天内到期发一次站内信（当日去重靠消息标题 + due_at 匹配）。
pub async fn bank_due_notify(db: &PgPool, days_before: i32) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, l.user_id, '银行贷款即将到期提醒',
               format('您的贷款（本金 %s 火花）将于 %s 到期，当前应结清 %s 火花（含计提利息），请及时还款以免逾期罚息。',
                      l.amount, to_char(l.due_at, 'YYYY-MM-DD'), l.remaining + l.accrued_interest)
        FROM bank_loans l
        WHERE l.status = 'active'
          AND l.due_at BETWEEN now() AND now() + ($1 || ' days')::interval
          AND NOT EXISTS (
            SELECT 1 FROM messages m
            WHERE m.receiver_id = l.user_id AND m.subject = '银行贷款即将到期提醒'
              AND m.created_at::date = CURRENT_DATE
          )
        "#,
    )
    .bind(days_before.to_string())
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// 银行每日结算总入口（worker 调用）。写健康游标供前端展示结息状态。
pub async fn bank_daily(db: &PgPool) {
    async fn setting(db: &PgPool, name: &str) -> Option<String> {
        sqlx::query_scalar::<_, String>("SELECT value FROM site_settings WHERE name = $1")
            .bind(name)
            .fetch_optional(db)
            .await
            .ok()
            .flatten()
    }
    let deduct_days: i32 = setting(db, "bank_auto_deduct_days")
        .await
        .and_then(|v| v.parse().ok())
        .unwrap_or(7);
    let allow_negative: bool = setting(db, "bank_allow_negative")
        .await
        .map(|v| v == "true")
        .unwrap_or(false);

    let mut demand_rows = 0u64;
    let mut fixed_rows = 0u64;
    let mut loan_rows = 0u64;
    let mut deduct_rows = 0u64;
    match bank_demand_settle(db).await {
        Ok(n) => demand_rows = n,
        Err(e) => tracing::error!(?e, "bank_demand_settle"),
    }
    match bank_fixed_daily_settle(db).await {
        Ok(n) => fixed_rows += n,
        Err(e) => tracing::error!(?e, "bank_fixed_daily_settle"),
    }
    match bank_fixed_mature(db).await {
        Ok(n) => fixed_rows += n,
        Err(e) => tracing::error!(?e, "bank_fixed_mature"),
    }
    match bank_loan_interest(db).await {
        Ok(n) => loan_rows = n,
        Err(e) => tracing::error!(?e, "bank_loan_interest"),
    }
    match bank_overdue_penalty(db).await {
        Ok(n) => loan_rows += n,
        Err(e) => tracing::error!(?e, "bank_overdue_penalty"),
    }
    match bank_auto_deduct(db, deduct_days, allow_negative).await {
        Ok(n) => deduct_rows = n,
        Err(e) => tracing::error!(?e, "bank_auto_deduct"),
    }
    if let Err(e) = bank_due_notify(db, 3).await {
        tracing::error!(?e, "bank_due_notify");
    }

    // 健康游标：当日结算完成时间 + 各任务行数（主键 run_date 天然幂等，重跑刷新计数）。
    // run_date 取站点日（UTC+8，与调度侧 site_day、API 健康页 (now()+8h)::date 同口径）；
    // 直接写 CURRENT_DATE（DB 为 UTC）会让 API 在整个站点日内查不到「今日已结」。
    let _ = sqlx::query(
        "INSERT INTO bank_settle_runs (run_date, demand_rows, fixed_rows, loan_rows, deduct_rows, finished_at) \
         VALUES ((CURRENT_TIMESTAMP + interval '8 hours')::date, $1, $2, $3, $4, now()) \
         ON CONFLICT (run_date) DO UPDATE SET \
           demand_rows = EXCLUDED.demand_rows, fixed_rows = EXCLUDED.fixed_rows, \
           loan_rows = EXCLUDED.loan_rows, deduct_rows = EXCLUDED.deduct_rows, \
           finished_at = EXCLUDED.finished_at",
    )
    .bind(demand_rows as i32)
    .bind(fixed_rows as i32)
    .bind(loan_rows as i32)
    .bind(deduct_rows as i32)
    .execute(db)
    .await;
}
