//! 活期与贷款（0048 火花银行对齐）。
//! 从 economy_http.rs 按域拆出。

use super::ledger::spend_spark_tx;
use super::spend::{earn_spark_tx, SpendOutcome};
use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::ok;
use crate::economy::DEMAND_RATE_BP;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 银行参数（site_settings 可配，缺省与 0048 迁移一致）
pub(super) struct BankSettings {
    pub(super) min_deposit: i64,
    pub(super) max_deposit: i64,
    pub(super) min_demand: i64,
    pub(super) loan_ratio: i64,
    pub(super) loan_constant: i64,
    pub(super) min_loan: i64,
    pub(super) demand_rate_bp: i32,
    pub(super) penalty_bp: i32,
    pub(super) overdue_penalty_bp: i32,
    #[allow(dead_code)] // worker 读 site_settings，API 侧仅透传给前端不需要
    pub(super) auto_deduct_days: i32,
    #[allow(dead_code)]
    pub(super) allow_negative: bool,
}

pub(super) async fn bank_settings(db: &PgPool) -> BankSettings {
    async fn get(db: &PgPool, name: &str) -> Option<String> {
        sqlx::query_scalar::<_, String>(
            "SELECT value FROM site_settings WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
    }
    BankSettings {
        min_deposit: get(db, "bank_min_deposit")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(100),
        max_deposit: get(db, "bank_max_deposit")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(1_000_000),
        min_demand: get(db, "bank_min_demand")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(100),
        loan_ratio: get(db, "bank_loan_ratio")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(100),
        loan_constant: get(db, "bank_loan_ratio_constant")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(1000),
        min_loan: get(db, "bank_min_loan")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(100),
        demand_rate_bp: get(db, "bank_demand_rate_bp")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEMAND_RATE_BP),
        penalty_bp: get(db, "bank_penalty_rate_bp")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(50),
        overdue_penalty_bp: get(db, "bank_overdue_penalty_bp")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(50),
        auto_deduct_days: get(db, "bank_auto_deduct_days")
            .await
            .and_then(|v| v.parse().ok())
            .unwrap_or(7),
        // yesno 门槛（admin 保存路径只认 yes/no；四审 S1 修正口径，历史
        // 种子值 "false" 一并按否处理）
        allow_negative: get(db, "bank_allow_negative")
            .await
            .map(|v| v == "yes")
            .unwrap_or(false),
    }
}

/// 经济路由（挂载进主 /api/v1 scope，单 scope 避免遮蔽）

// ============ 活期与贷款（0048 火花银行对齐） ============

#[derive(Deserialize)]
struct DemandDepositReq {
    amount: i64,
    /// 客户端幂等键（审计修复 P1：防网络重试双扣；缺省回退随机键保持兼容）
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/bank/demand/deposit")]
async fn demand_deposit(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DemandDepositReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.amount <= 0 {
        return Err(DomainError::Validation("存入金额必须为正".into()));
    }
    let bs = bank_settings(&state.repo.db).await;
    if body.amount < bs.min_demand {
        return Err(DomainError::Validation(format!(
            "活期单笔存入不少于 {} 魔力",
            bs.min_demand
        )));
    }
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .map(|k| format!("demand_in:{}:{}", auth.id, k.trim()))
        .unwrap_or_else(|| format!("demand_in:{}:{}", auth.id, Uuid::new_v4()));
    // 单事务（P1 撕裂窗口收口）：扣款与活期入账同生共死——旧版扣款先提交、入账失败
    // 只能靠 spawn 退款补偿（随机幂等键），进程崩溃窗口内钱扣了活期没涨。
    // 同时幂等重放必须终止（P0）：重放时 spend 不扣款，若继续累加活期余额 = 无中生有。
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
            "bank_demand_in",
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
    sqlx::query(
        "INSERT INTO bank_demand_accounts (user_id, balance, daily_rate_bp, last_interest_date) \
         VALUES ($1, $2, $3, CURRENT_DATE) \
         ON CONFLICT (user_id) DO UPDATE SET balance = bank_demand_accounts.balance + $2, \
         daily_rate_bp = $3, updated_at = now()",
    )
    .bind(auth.id)
    .bind(body.amount)
    .bind(bs.demand_rate_bp)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "deposited": body.amount })))
}

#[derive(Deserialize)]
struct DemandWithdrawReq {
    amount: i64,
    /// 客户端幂等键（审计修复 P1：防网络重试双发入账；缺省回退随机键保持兼容）
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/bank/demand/withdraw")]
async fn demand_withdraw(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DemandWithdrawReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.amount <= 0 {
        return Err(DomainError::Validation("支取金额必须为正".into()));
    }
    // 单事务（P1 撕裂窗口收口）：幂等检查、活期扣减、余额入账同生共死。旧版扣减先
    // 提交、earn 失败靠补偿 UPDATE 回补——进程崩溃窗口内钱从活期消失且无补偿；
    // 且幂等检查若后置于扣减，重试会再扣一次活期而入账侧被幂等键拦下 = 资金蒸发。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .map(|k| format!("demand_out:{}:{}", auth.id, k.trim()))
        .unwrap_or_else(|| {
            format!("demand_out:{}:{}", auth.id, Uuid::new_v4())
        });
    // 幂等检查前置到扣减之前（同事务内）：重放请求直接拒绝
    let replayed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
    )
    .bind(&idem)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if replayed {
        return Err(DomainError::Validation(
            "该笔支取已受理，请勿重复提交".into(),
        ));
    }
    let row: Option<i64> = sqlx::query_scalar(
        "UPDATE bank_demand_accounts SET balance = balance - $2, updated_at = now() \
         WHERE user_id = $1 AND balance >= $2 RETURNING balance",
    )
    .bind(auth.id)
    .bind(body.amount)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if row.is_none() {
        return Err(DomainError::Validation("活期余额不足".into()));
    }
    // 上方已在同事务内做过 EXISTS 重放预检，此处必为 Spent；显式丢弃以满足
    // must_use 契约（Replayed 只可能来自遗留撕裂残留，跳过入账即正确行为）
    let earn_outcome =
        earn_spark_tx(&mut tx, auth.id, body.amount, "bank_demand_out", &idem)
            .await?;
    let _ = earn_outcome;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "paid": body.amount, "balance_left": row }),
    ))
}

/// 最大可贷额度 = 时魔/小时 × 系数 + 常数（时魔取自近 1 小时做种收益口径，无则 0）
pub(super) async fn max_loan_amount(
    db: &PgPool,
    user_id: i64,
    bs: &BankSettings,
) -> DomainResult<i64> {
    let hourly: Option<i64> = sqlx::query_scalar(
        "SELECT COALESCE(sum(amount), 0)::bigint FROM spark_ledger \
         WHERE user_id = $1 AND kind = 'seeding_reward' AND created_at > now() - interval '1 hour'",
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(hourly.unwrap_or(0) * bs.loan_ratio + bs.loan_constant)
}
