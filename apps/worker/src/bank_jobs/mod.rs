//! 银行每日结算（0048 火花银行对齐）：活期复利结息、贷款计息、逾期罚息与
//! 自动扣款、定期到期自动结清、站内信通知。由 worker 日循环调度一次。
//!
//! 按域拆分：deposit（活期/定期结算）、loan（贷款计息/罚息/扣款/提醒）。

mod deposit;
mod loan;

pub use deposit::{
    bank_demand_settle, bank_fixed_daily_settle, bank_fixed_mature,
};
pub use loan::{
    bank_auto_deduct, bank_due_notify, bank_loan_interest, bank_overdue_penalty,
};

use sqlx::PgPool;

/// 银行每日结算总入口（worker 调用）。写健康游标供前端展示结息状态。
pub async fn bank_daily(db: &PgPool) {
    async fn setting(db: &PgPool, name: &str) -> Option<String> {
        sqlx::query_scalar::<_, String>(
            "SELECT value FROM site_settings WHERE name = $1",
        )
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
    // yesno 门槛（与 api 侧 loans.rs 同口径；历史种子值 "false" 按 false）
    let allow_negative: bool = setting(db, "bank_allow_negative")
        .await
        .map(|v| v == "yes")
        .unwrap_or(false);

    let mut demand_rows = 0u64;
    let mut fixed_rows = 0u64;
    let mut loan_rows = 0u64;
    let mut deduct_rows = 0u64;
    // 步骤失败标记：任一核心结算失败则不写健康游标（下一轮 init_bank_day 判
    // 「今日未结」整轮重跑——各子步骤自带 last_interest_date/幂等键游标，重跑安全）。
    // 旧口径失败后照写游标，当日利息永久少发且健康页仍显示「今日已结」。
    let mut failed = false;
    match bank_demand_settle(db).await {
        Ok(n) => demand_rows = n,
        Err(e) => {
            tracing::error!(?e, "bank_demand_settle");
            failed = true;
        }
    }
    match bank_fixed_daily_settle(db).await {
        Ok(n) => fixed_rows += n,
        Err(e) => {
            tracing::error!(?e, "bank_fixed_daily_settle");
            failed = true;
        }
    }
    match bank_fixed_mature(db).await {
        Ok(n) => fixed_rows += n,
        Err(e) => {
            tracing::error!(?e, "bank_fixed_mature");
            failed = true;
        }
    }
    match bank_loan_interest(db).await {
        Ok(n) => loan_rows = n,
        Err(e) => {
            tracing::error!(?e, "bank_loan_interest");
            failed = true;
        }
    }
    match bank_overdue_penalty(db).await {
        Ok(n) => loan_rows += n,
        Err(e) => {
            tracing::error!(?e, "bank_overdue_penalty");
            failed = true;
        }
    }
    match bank_auto_deduct(db, deduct_days, allow_negative).await {
        Ok(n) => deduct_rows = n,
        Err(e) => {
            tracing::error!(?e, "bank_auto_deduct");
            failed = true;
        }
    }
    if let Err(e) = bank_due_notify(db, 3).await {
        tracing::error!(?e, "bank_due_notify");
    }
    if failed {
        tracing::error!(
            "bank_daily finished with failures; health cursor NOT advanced, will retry next tick"
        );
        return;
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
