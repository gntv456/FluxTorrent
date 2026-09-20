//! 活期与定期结算：活期复利结息、定期每日结息、定期到期自动结清。

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
    // rate 是 NUMERIC，sqlx 不直接解 NUMERIC→f64，取 text 自行解析；
    // maturity 一并取回，供循环内把结算终点截断到到期日（防到期后补跑多付利息）
    let rows: Vec<(i64, i64, i64, String, chrono::NaiveDate, chrono::NaiveDate)> = sqlx::query_as(
        "SELECT id, user_id, amount, rate::text, COALESCE(last_interest_date, (start_at::date - 1)), maturity_at::date \
         FROM bank_deposits \
         WHERE status = 0 AND settle_mode = 'daily' \
           AND COALESCE(last_interest_date, (start_at::date - 1)) < LEAST(CURRENT_DATE, maturity_at::date) \
         LIMIT 500",
    )
    .fetch_all(db)
    .await?;
    let mut count = 0u64;
    for (id, user_id, amount, rate_text, last_date, maturity) in rows {
        let annual_rate: f64 = rate_text.parse().unwrap_or(0.0);
        // 审计修复（末日利息）：结算终点截断到 LEAST(今天, maturity)——到期后 worker
        // 补跑/重试时不再按「今天」多计到期日至补跑日之间的利息（多付部分无法追回）。
        // （SELECT 侧已用 LEAST 过滤欠结行，此处是对「游标落后跨过到期日」的行兜底。）
        let today = chrono::Utc::now().date_naive();
        let end = if maturity < today { maturity } else { today };
        let days = end.signed_duration_since(last_date).num_days().max(0);
        if days == 0 {
            continue;
        }
        // 年化 → 日息，向下取整防超发；与 maturity_interest 全期口径一致
        let interest = ((amount as f64) * annual_rate * (days as f64) / 365.0)
            .floor() as i64;
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
                let balance: i64 = sqlx::query_scalar(
                    "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
                )
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
            let balance: i64 = sqlx::query_scalar(
                "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
            )
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
                    "您的定期存款已到期，本金 {amount} 魔力已返还；存期内已按日发放利息共 {paid_interest} 魔力。"
                )
            } else {
                format!(
                    "您的定期存款已到期，本金 {amount} 魔力与利息 {interest} 魔力已自动返还到您的账户。"
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
