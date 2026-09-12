//! 银行每日结算（0048 火花银行对齐）：活期复利结息、贷款计息、逾期罚息与
//! 自动扣款、定期到期自动结清、站内信通知。由 worker 日循环调度一次。

use sqlx::PgPool;

/// 活期结息：利息滚入活期本金（复利）+ 利息明细。
/// last_interest_date < 今天 的账户按天数补结；幂等靠日期游标。
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
            WHERE a.id = c.id AND c.interest > 0
            RETURNING a.id
        )
        INSERT INTO bank_interest_records (user_id, kind, reference_id, amount, rate_bp)
        SELECT c.user_id, 'demand', c.id, c.interest, c.daily_rate_bp
        FROM calc c JOIN upd ON upd.id = c.id
        "#,
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// 贷款正常期计息：利息只计提不并本金（结清时一次收），记明细。
/// 计息截止 min(今天, 到期日)；当日入账口径（首日计 1 天）。
pub async fn bank_loan_interest(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        WITH due AS (
            SELECT l.id, l.user_id, l.remaining, l.daily_rate_bp, l.last_interest_date,
                   LEAST(CURRENT_DATE, l.due_at::date) AS end_date
            FROM bank_loans l
            WHERE l.status = 'active'
              AND COALESCE(l.last_interest_date, (l.created_at::date - 1)) < LEAST(CURRENT_DATE, l.due_at::date)
        ),
        calc AS (
            SELECT id, user_id, daily_rate_bp,
                   (remaining * daily_rate_bp *
                     (end_date - COALESCE(last_interest_date, (created_at::date - 1))) / 10000)::bigint
                     AS interest
            FROM bank_loans l JOIN due ON due.id = l.id
        ),
        upd AS (
            UPDATE bank_loans l
            SET accrued_interest = l.accrued_interest + c.interest,
                last_interest_date = (SELECT LEAST(CURRENT_DATE, l.due_at::date))
            FROM calc c
            WHERE l.id = c.id AND c.interest > 0
            RETURNING l.id
        )
        INSERT INTO bank_interest_records (user_id, kind, reference_id, amount, rate_bp)
        SELECT c.user_id, 'loan', c.id, -c.interest, c.daily_rate_bp
        FROM calc c JOIN upd ON upd.id = c.id
        "#,
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// 逾期罚息：逾期后每日按罚息率计息并并入本金（与火花口径一致）。
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
                   (remaining * penalty_rate_bp * days / 10000)::bigint AS penalty
            FROM due
        ),
        upd AS (
            UPDATE bank_loans l
            SET remaining = l.remaining + c.penalty,
                last_interest_date = CURRENT_DATE
            FROM calc c
            WHERE l.id = c.id AND c.penalty > 0
            RETURNING l.id
        )
        INSERT INTO bank_interest_records (user_id, kind, reference_id, amount, rate_bp)
        SELECT c.user_id, 'loan_penalty', c.id, -c.penalty, c.penalty_rate_bp
        FROM calc c JOIN upd ON upd.id = c.id
        "#,
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// 严重逾期自动扣款：逾期超 N 天，先扣活期再扣站内余额，扣清为止。
/// 负余额口径走站点设置 bank_allow_negative；每次调度最多扣一次（游标 = updated_at 当日已处理）。
pub async fn bank_auto_deduct(db: &PgPool, deduct_days: i32, allow_negative: bool) -> anyhow::Result<u64> {
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
        // 先扣活期
        let from_demand: i64 = sqlx::query_scalar(
            "UPDATE bank_demand_accounts SET balance = balance - LEAST(balance, $3), updated_at = now() \
             WHERE user_id = $2 RETURNING LEAST(balance, $3)",
        )
        .bind(loan_id)
        .bind(user_id)
        .bind(remaining)
        .fetch_optional(&mut *tx)
        .await?
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
                let balance: i64 = sqlx::query_scalar(
                    "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
                )
                .bind(user_id)
                .fetch_one(&mut *tx)
                .await?;
                let take = balance.min(left).max(0);
                if take > 0 {
                    let idem = format!("auto_deduct:{}:{}", loan_id, chrono::Utc::now().format("%Y%m%d"));
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
                        sqlx::query("UPDATE users SET spark_balance = spark_balance - $2 WHERE id = $1")
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

/// 定期到期自动结清：本息入余额（幂等键 = withdraw:{deposit_id}，与手动支取同键防双发）。
pub async fn bank_fixed_mature(db: &PgPool) -> anyhow::Result<u64> {
    let rows: Vec<(i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT id, user_id, amount, interest FROM bank_deposits \
         WHERE status = 0 AND maturity_at <= now() LIMIT 500",
    )
    .fetch_all(db)
    .await?;
    let mut count = 0u64;
    for (id, user_id, amount, interest) in rows {
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
            let balance: i64 = sqlx::query_scalar(
                "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
            )
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
            let payable = amount + interest;
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
            if interest > 0 {
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
            sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
            )
            .bind(user_id)
            .bind("银行存款到期通知")
            .bind(format!(
                "您的定期存款已到期，本金 {amount} 火花与利息 {interest} 火花已自动返还到您的账户。"
            ))
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

/// 银行每日结算总入口（worker 调用）。
pub async fn bank_daily(db: &PgPool) {
    let deduct_days: i32 = sqlx::query_scalar::<_, String>("SELECT value FROM site_settings WHERE name = 'bank_auto_deduct_days'")
        .fetch_optional(db).await.ok().flatten()
        .and_then(|v| v.parse().ok()).unwrap_or(7);
    let allow_negative: bool = sqlx::query_scalar::<_, String>("SELECT value FROM site_settings WHERE name = 'bank_allow_negative'")
        .fetch_optional(db).await.ok().flatten()
        .map(|v| v == "true").unwrap_or(false);

    if let Err(e) = bank_demand_settle(db).await { tracing::error!(?e, "bank_demand_settle"); }
    if let Err(e) = bank_fixed_mature(db).await { tracing::error!(?e, "bank_fixed_mature"); }
    if let Err(e) = bank_loan_interest(db).await { tracing::error!(?e, "bank_loan_interest"); }
    if let Err(e) = bank_overdue_penalty(db).await { tracing::error!(?e, "bank_overdue_penalty"); }
    if let Err(e) = bank_auto_deduct(db, deduct_days, allow_negative).await { tracing::error!(?e, "bank_auto_deduct"); }
    if let Err(e) = bank_due_notify(db, 3).await { tracing::error!(?e, "bank_due_notify"); }
}
