//! 银行（M11）：定存存取。
//! 从 economy_http.rs 按域拆出。

use super::ledger::spend_spark_tx;
use super::loans::bank_settings;
use super::spend::SpendOutcome;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use uuid::Uuid;

use crate::dto::ok;
use crate::economy::{maturity_interest_with_rules, term_rate_with_rules, VALID_TERMS};
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 经济路由（挂载进主 /api/v1 scope，单 scope 避免遮蔽）

// ============ 银行（M11） ============

#[derive(Deserialize)]
struct DepositReq {
    amount: i64,
    term_days: i32,
    /// 客户端幂等键（审计修复 P1：服务端随机键导致网络重试/双击=双扣款。
    /// 与 shop/funding 同口径：重试必须携带同一键；缺省回退随机键保持兼容）
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/bank/deposit")]
async fn bank_deposit(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DepositReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if !VALID_TERMS.contains(&body.term_days) {
        return Err(DomainError::Validation(
            "期限仅支持 7/30/90/180/365 天".into(),
        ));
    }
    if body.amount <= 0 {
        return Err(DomainError::Validation("存款金额必须为正".into()));
    }
    let bs = bank_settings(&state.repo.db).await;
    if body.amount < bs.min_deposit {
        return Err(DomainError::Validation(format!(
            "定期存款单笔不少于 {} 魔力",
            bs.min_deposit
        )));
    }
    if bs.max_deposit > 0 && body.amount > bs.max_deposit {
        return Err(DomainError::Validation(format!(
            "定期存款单笔不可超过 {} 魔力",
            bs.max_deposit
        )));
    }
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .map(|k| format!("deposit:{}:{}", auth.id, k.trim()))
        .unwrap_or_else(|| format!("deposit:{}:{}", auth.id, Uuid::new_v4()));
    let interest =
        maturity_interest_with_rules(&state.repo.db, body.amount, body.term_days)
            .await;
    // 结息模式：daily = 每日结息发到余额（到期只还本）；maturity = 到期一次性
    let mode: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'bank_fixed_settle_mode'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or_else(|| "maturity".into());
    let mode = if mode == "daily" { "daily" } else { "maturity" };
    // 单事务（P1 撕裂窗口收口）：扣款与存单落库同生共死。旧版扣款先提交、存单
    // INSERT 失败只能靠 spawn 退款补偿（随机幂等键），进程崩溃窗口内钱扣了无存单、
    // 无对账记录。同时幂等重放必须终止（P0）：重放时 spend 不再扣款，若继续落存单
    // = 免费多得一张存单、到期照付本息（对照 games scratch 的同款闸门）。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if !matches!(
        spend_spark_tx(
            &mut tx,
            auth.id,
            body.amount,
            "bank_deposit",
            &idem,
            "bank",
            0,
        )
        .await?,
        SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "该笔请求已受理，请勿重复提交".into(),
        ));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO bank_deposits (user_id, amount, term_days, rate, interest, maturity_at, settle_mode) \
         VALUES ($1, $2, $3, $4, $5, now() + ($3 || ' days')::interval, $6) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.amount)
    .bind(body.term_days)
    .bind(term_rate_with_rules(&state.repo.db, body.term_days).await)
    .bind(interest)
    .bind(mode)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    Ok(ok(serde_json::json!({
        "id": id, "amount": body.amount, "term_days": body.term_days,
        "interest": interest, "rate": term_rate_with_rules(&state.repo.db, body.term_days).await, "settle_mode": mode,
    })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct DepositRow {
    id: i64,
    amount: i64,
    term_days: i32,
    interest: i64,
    paid_interest: i64,
    settle_mode: String,
    status: i16,
    maturity_at: chrono::DateTime<chrono::Utc>,
}

#[get("/bank/deposits")]
async fn bank_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, DepositRow>(
        "SELECT id, amount, term_days, interest, paid_interest, settle_mode, status, maturity_at FROM bank_deposits \
         WHERE user_id = $1 ORDER BY id DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}
