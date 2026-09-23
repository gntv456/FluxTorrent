//! 贷款还款（0048）。
//! 从 economy_http/loans_apply.rs 按域拆出。

use super::ledger::spend_spark_tx;
use super::loans::{bank_settings, max_loan_amount};
use super::spend::SpendOutcome;
use crate::dto::ok;
use crate::economy;
use crate::economy::{loan_rate_bp, term_rate, LOAN_TERMS, VALID_TERMS};
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};

#[derive(serde::Deserialize, Default)]
struct RepayReq {
    /// 部分还款金额（魔力）。缺省/0 = 一次性结清（兼容旧客户端）
    #[serde(default)]
    amount: Option<i64>,
    /// 客户端幂等键：部分还款可多次提交，重试必须携带同一键防双扣
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/bank/loan/repay")]
async fn loan_repay(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: Option<web::Json<RepayReq>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let body = body.map(|b| b.into_inner()).unwrap_or_default();
    let partial = body.amount.filter(|a| *a > 0);
    // 单事务（P1 收口）：旧版 FOR UPDATE 用在 autocommit 连接上——语句结束锁即释放，
    // payoff 按陈旧应计利息计算；扣款/销账两段提交靠退款补偿。现在锁、算、扣、销同事务。
    // 锁序先 users 后 loan（与 worker bank_auto_deduct 的 users→loan 一致，防互锁）。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let _user_lock: i64 = sqlx::query_scalar(
        "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(auth.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 结清额 = 剩余本金 + 计提至今利息（含当日，一次性结清）
    //（daily_rate_bp 是 INT4：显式 cast 防 query_as 解码 i64 报类型不匹配）
    let loan: Option<(i64, i64, i64, i32, chrono::NaiveDate)> = sqlx::query_as(
        "SELECT id, remaining, accrued_interest, daily_rate_bp::int, last_interest_date \
         FROM bank_loans WHERE user_id = $1 AND status IN ('active', 'defaulted') FOR UPDATE",
    )
    .bind(auth.id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((id, remaining, accrued, rate_bp, last_date)) = loan else {
        return Err(DomainError::Validation("没有进行中的贷款".into()));
    };
    let days = (chrono::Utc::now().date_naive() - last_date).num_days();
    let today_interest = if days > 0 {
        economy::loan_interest(remaining, rate_bp, days)
    } else {
        0
    };
    let payoff = remaining + accrued + today_interest;
    // 部分还款（D 缺口）：先息后本——付不掉全额时按「今日+累计利息 → 本金」顺序冲抵，
    // 余额不足报错。全额结清走原路径（销账+status='paid'）。
    let (paid_total, principal_paid, interest_paid, settled_all) = match partial {
        Some(amount) if amount < payoff => {
            if amount < 0 {
                return Err(DomainError::Validation(
                    "还款金额必须为正".into(),
                ));
            }
            let to_interest = amount.min(accrued + today_interest);
            let to_principal = amount - to_interest;
            let idem = body
                .idempotency_key
                .clone()
                .filter(|k| !k.trim().is_empty())
                .map(|k| format!("loan_repay_p:{}:{}", auth.id, k.trim()))
                .ok_or_else(|| {
                    DomainError::Validation("部分还款缺少幂等键".into())
                })?;
            if !matches!(
                spend_spark_tx(
                    &mut tx,
                    auth.id,
                    amount,
                    "bank_loan_repay_partial",
                    &idem,
                    "bank",
                    id,
                )
                .await?,
                SpendOutcome::Spent
            ) {
                return Err(DomainError::Validation(
                    "该笔还款已受理，请勿重复提交".into(),
                ));
            }
            // 利息冲抵顺序：今日计提先于往期累计（时序上更「新」的债务先清）
            let today_deduct = today_interest.min(to_interest);
            let accrued_deduct = to_interest - today_deduct;
            if accrued_deduct > 0 {
                sqlx::query(
                    "UPDATE bank_loans SET accrued_interest = accrued_interest - $2 \
                     WHERE id = $1",
                )
                .bind(id)
                .bind(accrued_deduct)
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            }
            let new_remaining = remaining - to_principal;
            if to_principal > 0 {
                sqlx::query(
                    "UPDATE bank_loans SET remaining = $2, \
                     last_interest_date = CURRENT_DATE WHERE id = $1",
                )
                .bind(id)
                .bind(new_remaining)
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            } else if today_deduct > 0 {
                // 只还息不动本：计息游标也要推到今天，否则明日按双倍天数补计今日利息
                sqlx::query(
                    "UPDATE bank_loans SET last_interest_date = CURRENT_DATE \
                     WHERE id = $1",
                )
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            }
            (amount, to_principal, to_interest, false)
        }
        _ => {
            let idem = format!("loan_repay:{}", id);
            // 幂等重放闸门（#[must_use] 连审）：重放不重复扣款
            if !matches!(
                spend_spark_tx(
                    &mut tx,
                    auth.id,
                    payoff,
                    "bank_loan_repay",
                    &idem,
                    "bank",
                    id,
                )
                .await?,
                SpendOutcome::Spent
            ) {
                return Err(DomainError::Validation(
                    "该笔还款已受理，请勿重复提交".into(),
                ));
            }
            // 销账校验影响行数（审计 P1）：与 worker bank_auto_deduct 并发时贷款可能已被结清。
            // 单事务下销账 0 行 = 整体回滚（扣款一并撤销），不再需要退款补偿路径。
            let settled = sqlx::query(
                "UPDATE bank_loans SET remaining = 0, accrued_interest = 0, status = 'paid', \
                 paid_at = now(), last_interest_date = CURRENT_DATE WHERE id = $1 AND status IN ('active', 'defaulted')",
            )
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
            if settled == 0 {
                return Err(DomainError::Validation(
                    "贷款状态已变更（可能已被系统自动扣款结清），请刷新后重试".into(),
                ));
            }
            (payoff, remaining, accrued + today_interest, true)
        }
    };
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "paid": paid_total, "principal": principal_paid,
        "interest": interest_paid, "settled": settled_all,
        "remaining": payoff - paid_total,
    })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct BankLoanRow {
    id: i64,
    amount: i64,
    daily_rate_bp: i32,
    term_days: i32,
    remaining: i64,
    accrued_interest: i64,
    status: String,
    due_at: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct DemandRow {
    balance: i64,
    daily_rate_bp: i32,
    last_interest_date: Option<chrono::NaiveDate>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct InterestRecordRow {
    id: i64,
    kind: String,
    reference_id: i64,
    amount: i64,
    rate_bp: i32,
    calc_date: chrono::NaiveDate,
}

/// 个人利息流水（demand 活期 / fixed 定期 / loan 贷款计提，近 50 条）
#[get("/bank/interest/records")]
async fn bank_interest_records(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, InterestRecordRow>(
        "SELECT id, kind, reference_id, amount, rate_bp, calc_date \
         FROM bank_interest_records WHERE user_id = $1 \
         ORDER BY calc_date DESC, id DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 银行总览：活期账户 + 资产汇总 + 当前贷款 + 额度
#[get("/bank/overview")]
async fn bank_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let bs = bank_settings(&state.repo.db).await;

    let demand: DemandRow = sqlx::query_as(
        "SELECT balance, daily_rate_bp, \
         last_interest_date FROM bank_demand_accounts WHERE user_id = $1",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or(DemandRow {
        balance: 0,
        daily_rate_bp: bs.demand_rate_bp,
        last_interest_date: None,
    });

    let fixed: Option<(i64, i64)> = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COALESCE(sum(amount), 0)::bigint, \
         count(*) FROM bank_deposits WHERE user_id = $1 AND status = 0",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let loan: Option<BankLoanRow> = sqlx::query_as(
        // defaulted（逾期半期未还）也展示：用户需能看到被标记违约的贷款并还款
        "SELECT id, amount, daily_rate_bp, term_days, remaining, accrued_interest, status, due_at \
         FROM bank_loans WHERE user_id = $1 AND status IN ('active', 'defaulted')",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let spark: i64 =
        sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

    let loan_outstanding = loan
        .as_ref()
        .map(|l| l.remaining + l.accrued_interest)
        .unwrap_or(0);
    let total_asset = spark + demand.balance + fixed.unwrap_or((0, 0)).0;
    let max_loan = max_loan_amount(&state.repo.db, auth.id, &bs).await?;

    // 站点级运营概览 + 结息健康状态（对齐火花「站点银行概览/结息状态」）
    let site: (i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT \
           (SELECT COALESCE(sum(balance), 0)::bigint FROM bank_demand_accounts), \
           (SELECT count(*) FROM bank_demand_accounts WHERE balance > 0), \
           (SELECT COALESCE(sum(amount), 0)::bigint FROM bank_deposits WHERE status = 0), \
           (SELECT count(*) FROM bank_deposits WHERE status = 0), \
           (SELECT COALESCE(sum(remaining + accrued_interest), 0)::bigint FROM bank_loans WHERE status = 'active'), \
           (SELECT count(*) FROM bank_loans WHERE status = 'active'), \
           (SELECT count(*) FROM bank_interest_records WHERE calc_date = CURRENT_DATE)",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let last_run: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT finished_at FROM bank_settle_runs WHERE run_date = \
         ((CURRENT_TIMESTAMP + interval '8 hours')::date)",
    )
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let settle_mode: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'bank_fixed_settle_mode'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or_else(|| "maturity".into());

    Ok(ok(serde_json::json!({
        "spark_balance": spark,
        "demand": { "balance": demand.balance, "daily_rate_bp": demand.daily_rate_bp },
        "fixed": { "active_total": fixed.map(|f| f.0).unwrap_or(0), "active_count": fixed.map(|f| f.1).unwrap_or(0) },
        "loan": loan,
        "total_asset": total_asset,
        "net_asset": total_asset - loan_outstanding,
        "loan_outstanding": loan_outstanding,
        "max_loan": max_loan,
        "limits": {
            "min_deposit": bs.min_deposit, "max_deposit": bs.max_deposit,
            "min_demand": bs.min_demand, "min_loan": bs.min_loan,
            "penalty_bp": bs.penalty_bp,
        },
        "site": {
            "demand_total": site.0, "demand_count": site.1,
            "fixed_active_total": site.2, "fixed_count": site.3,
            "loan_outstanding_total": site.4, "loan_count": site.5,
            "today_interest_records": site.6,
            "settle_healthy": last_run.is_some(),
            "settle_mode": settle_mode,
        },
        "fixed_rates": VALID_TERMS.iter().map(|t| serde_json::json!({
            "term_days": t, "annual_rate": term_rate(*t),
        })).collect::<Vec<_>>(),
        "loan_rates": LOAN_TERMS.iter().map(|t| serde_json::json!({
            "term_days": t, "daily_rate_bp": loan_rate_bp(*t),
        })).collect::<Vec<_>>(),
    })))
}

// ============ 签到（M12） ============
