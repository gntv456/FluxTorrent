//! 贷款申请与还款（0048）。
//! 从 economy_http.rs 按域拆出。

use super::loans::{bank_settings, max_loan_amount};
use super::spend::earn_spark_tx;
use crate::dto::ok;
use crate::economy::{loan_rate_bp, LOAN_TERMS};
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

#[post("/bank/loan/apply")]
async fn loan_apply(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<LoanApplyReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if !LOAN_TERMS.contains(&body.term_days) {
        return Err(DomainError::Validation(
            "贷款期限仅支持 7/30/90/180/365 天".into(),
        ));
    }
    let bs = bank_settings(&state.repo.db).await;
    if body.amount < bs.min_loan {
        return Err(DomainError::Validation(format!(
            "贷款金额不少于 {} 魔力",
            bs.min_loan
        )));
    }
    let balance: i64 =
        sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if balance < 0 {
        return Err(DomainError::Validation(
            "当前魔力为负，暂不可申请贷款".into(),
        ));
    }
    let max = max_loan_amount(&state.repo.db, auth.id, &bs).await?;
    if body.amount > max {
        return Err(DomainError::Validation(format!(
            "贷款金额不可超过额度上限 {} 魔力（时魔 × {} + {}）",
            max, bs.loan_ratio, bs.loan_constant
        )));
    }
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM bank_loans WHERE user_id = $1 AND \
         status IN ('active', 'defaulted'))",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if active {
        return Err(DomainError::Validation(
            "已有未结清贷款，请先结清后再申请".into(),
        ));
    }
    let rate = loan_rate_bp(body.term_days);
    // 单事务（P1 撕裂窗口收口）：放款与建贷款行同生共死——旧版 earn 失败靠 DELETE
    // 补偿回滚贷款行，进程崩溃窗口内残留 active 贷款进入计息/逾期/自动扣款集合
    //（用户没收到钱却背上了债务）。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO bank_loans (user_id, amount, daily_rate_bp, penalty_rate_bp, term_days, remaining, due_at, last_interest_date) \
         VALUES ($1, $2, $3, $4, $5, $2, now() + ($5 || ' days')::interval, CURRENT_DATE) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.amount)
    .bind(rate)
    .bind(bs.overdue_penalty_bp)
    .bind(body.term_days)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let idem = format!("loan:{}:{}", auth.id, id);
    // 幂等键含新建贷款 id，重放不可达；显式丢弃以满足 must_use 契约
    let earn_outcome =
        earn_spark_tx(&mut tx, auth.id, body.amount, "bank_loan_payout", &idem)
            .await?;
    let _ = earn_outcome;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "id": id, "amount": body.amount, "term_days": body.term_days, "daily_rate_bp": rate,
        "due_in_days": body.term_days,
    })))
}

#[derive(Deserialize)]
pub(super) struct LoanApplyReq {
    pub(super) amount: i64,
    pub(super) term_days: i32,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct LoanHistoryRow {
    id: i64,
    amount: i64,
    daily_rate_bp: i32,
    term_days: i32,
    remaining: i64,
    accrued_interest: i64,
    status: String,
    due_at: chrono::DateTime<chrono::Utc>,
    paid_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// 贷款历史（进行中 + 已结清，近 50 笔）。定存有 /bank/deposits 对应物。
#[get("/bank/loans")]
async fn loan_history(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl actix_web::Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, LoanHistoryRow>(
        "SELECT id, amount, daily_rate_bp, term_days, remaining, accrued_interest, \
         status, due_at, paid_at, created_at FROM bank_loans \
         WHERE user_id = $1 ORDER BY id DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}
