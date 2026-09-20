//! 贷款计息与逾期处理：正常期计息、逾期罚息、自动扣款、到期提醒。

use sqlx::PgPool;

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
    // 外层 remaining 只是事务外快照（只用于筛选），实际扣款以锁内重读的为准
    for (loan_id, user_id, _snapshot_remaining) in loans {
        let mut tx = db.begin().await?;
        // 当日幂等闸门：同一贷款同一天只允许扣一次（含活期+余额两段）。
        // 幂等键与流水共用 auto_deduct:{loan}:{YYYYMMDD}；当日已扣过则整笔跳过。
        let idem = format!(
            "auto_deduct:{}:{}",
            loan_id,
            chrono::Utc::now().format("%Y%m%d")
        );
        let already: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
        )
        .bind(&idem)
        .fetch_one(&mut *tx)
        .await?;
        if already {
            tx.rollback().await?;
            continue;
        }
        // 锁内重读贷款状态与剩余本金（P1 与手动还款并发）：外层 SELECT 是事务外快照，
        // 用户并发 loan_repay 结清后仍按陈旧 remaining 扣款且最后更新无 status 过滤
        // = 已结清贷款被再扣一次。status 非 active/defaulted 直接跳过本笔。
        let (locked_remaining, locked_status): (i64, String) = sqlx::query_as(
            "SELECT remaining, status FROM bank_loans WHERE id = $1 FOR UPDATE",
        )
        .bind(loan_id)
        .fetch_one(&mut *tx)
        .await?;
        if locked_status != "active" && locked_status != "defaulted" {
            tx.rollback().await?;
            continue;
        }
        let remaining = locked_remaining;
        // 先扣活期：CTE 先锁定并取「更新前」余额再更新，实扣额 = LEAST(前余额, 欠款)。
        // （RETURNING 里直接算 before 会用更新后的余额重算 LEAST，旧余额>欠款时低报、
        //   少算的差额又被继续从站内余额扣 → 系统性多扣。）
        let from_demand: i64 = sqlx::query_scalar(
            "WITH prev AS (SELECT balance FROM bank_demand_accounts WHERE user_id = $1 FOR UPDATE) \
             UPDATE bank_demand_accounts a SET balance = a.balance - LEAST((SELECT balance FROM prev), $2), updated_at = now() \
             WHERE a.user_id = $1 RETURNING LEAST((SELECT balance FROM prev), $2)",
        )
        .bind(user_id)
        .bind(remaining)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);
        let left = remaining - from_demand;
        let mut deducted = from_demand;
        if left > 0 {
            if allow_negative {
                // 负余额口径：直接扣清（上方幂等闸门已保证当日只扣一次）
                sqlx::query(
                    "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key) \
                     VALUES (nextval('spark_ledger_id_seq'), $2, -$3, 'bank_auto_deduct', 'bank', $1, $4)",
                )
                .bind(loan_id)
                .bind(user_id)
                .bind(left)
                .bind(&idem)
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
                     WHERE id = $1 AND status IN ('active', 'defaulted')",
                )
                .bind(loan_id)
                .execute(&mut *tx)
                .await?;
            } else {
                sqlx::query(
                    "UPDATE bank_loans SET remaining = $2 WHERE id = $1 AND status IN ('active', 'defaulted')",
                )
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
                "您的贷款已逾期，系统自动扣款 {deducted} 魔力（活期 {from_demand}）。剩余欠款 {}。请尽快结清，逾期期间每日加计罚息。",
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

/// 到期前提醒：贷款 3 天内到期发一次站内信（当日去重靠消息标题 + due_at 匹配）。
/// 审计修复（去重失真）：subject 带贷款 id——旧标题全站同一字符串，NOT EXISTS 按
/// 「该用户当日已有过任意一条到期提醒」去重，多笔贷款同时到期时只提醒第一笔。
/// 改为 `...提醒 #{loan_id}` 且 NOT EXISTS 匹配同 subject 当日，每笔贷款各自去重。
pub async fn bank_due_notify(
    db: &PgPool,
    days_before: i32,
) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, l.user_id, '银行贷款即将到期提醒 #' || l.id,
               format('您的贷款（本金 %s 魔力）将于 %s 到期，当前应结清 %s 魔力（含计提利息），请及时还款以免逾期罚息。',
                      l.amount, to_char(l.due_at, 'YYYY-MM-DD'), l.remaining + l.accrued_interest)
        FROM bank_loans l
        WHERE l.status = 'active'
          AND l.due_at BETWEEN now() AND now() + ($1 || ' days')::interval
          AND NOT EXISTS (
            SELECT 1 FROM messages m
            WHERE m.receiver_id = l.user_id
              AND m.subject = '银行贷款即将到期提醒 #' || l.id
              AND m.created_at::date = CURRENT_DATE
          )
        "#,
    )
    .bind(days_before.to_string())
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}
