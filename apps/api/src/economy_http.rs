//! 经济系统 HTTP 接口（M11 商店/流水/银行 + M12 签到 + M13 站免池）。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::ok;
use crate::economy::{
    self, checkin_reward, early_penalty, loan_rate_bp, maturity_interest, term_rate, DEMAND_RATE_BP,
    LOAN_TERMS, VALID_TERMS,
};
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 银行参数（site_settings 可配，缺省与 0048 迁移一致）
struct BankSettings {
    min_deposit: i64,
    max_deposit: i64,
    min_demand: i64,
    loan_ratio: i64,
    loan_constant: i64,
    min_loan: i64,
    demand_rate_bp: i32,
    penalty_bp: i32,
    overdue_penalty_bp: i32,
    #[allow(dead_code)] // worker 读 site_settings，API 侧仅透传给前端不需要
    auto_deduct_days: i32,
    #[allow(dead_code)]
    allow_negative: bool,
}

async fn bank_settings(db: &PgPool) -> BankSettings {
    async fn get(db: &PgPool, name: &str) -> Option<String> {
        sqlx::query_scalar::<_, String>("SELECT value FROM site_settings WHERE name = $1")
            .bind(name)
            .fetch_optional(db)
            .await
            .ok()
            .flatten()
    }
    BankSettings {
        min_deposit: get(db, "bank_min_deposit").await.and_then(|v| v.parse().ok()).unwrap_or(100),
        max_deposit: get(db, "bank_max_deposit").await.and_then(|v| v.parse().ok()).unwrap_or(1_000_000),
        min_demand: get(db, "bank_min_demand").await.and_then(|v| v.parse().ok()).unwrap_or(100),
        loan_ratio: get(db, "bank_loan_ratio").await.and_then(|v| v.parse().ok()).unwrap_or(100),
        loan_constant: get(db, "bank_loan_ratio_constant").await.and_then(|v| v.parse().ok()).unwrap_or(1000),
        min_loan: get(db, "bank_min_loan").await.and_then(|v| v.parse().ok()).unwrap_or(100),
        demand_rate_bp: get(db, "bank_demand_rate_bp").await.and_then(|v| v.parse().ok()).unwrap_or(DEMAND_RATE_BP),
        penalty_bp: get(db, "bank_penalty_rate_bp").await.and_then(|v| v.parse().ok()).unwrap_or(50),
        overdue_penalty_bp: get(db, "bank_overdue_penalty_bp").await.and_then(|v| v.parse().ok()).unwrap_or(50),
        auto_deduct_days: get(db, "bank_auto_deduct_days").await.and_then(|v| v.parse().ok()).unwrap_or(7),
        allow_negative: get(db, "bank_allow_negative").await.map(|v| v == "true").unwrap_or(false),
    }
}

/// 经济路由（挂载进主 /api/v1 scope，单 scope 避免遮蔽）
pub fn mount_economy(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(shop_items)
        .service(shop_buy)
        .service(my_spark)
        .service(my_ledger)
        .service(bank_deposit)
        .service(bank_withdraw)
        .service(bank_list)
        .service(bank_overview)
        .service(demand_deposit)
        .service(demand_withdraw)
        .service(loan_apply)
        .service(loan_repay)
        .service(checkin)
        .service(checkin_status)
        .service(pool_status)
        .service(pool_donate)
        .service(dressup_list)
        .service(dressup_wear)
}

/// 动账核心：余额充足校验 + 负流水 + 余额快照更新（单事务）。
/// 幂等键唯一约束（shop_orders/应用层先查）防重复扣款。
/// 扣款结果：区分真实扣款与幂等重放（调用方据此决定是否执行副作用）
pub enum SpendOutcome {
    Spent,
    Replayed,
}

pub async fn spend_spark(
    db: &PgPool,
    user_id: i64,
    amount: i64,
    kind: &str,
    idem: &str,
    ref_type: &str,
    ref_id: i64,
) -> DomainResult<SpendOutcome> {
    let mut tx = db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等检查必须在余额检查之前：已成功扣过的键在余额不足时也应返回重放，
    // 而不是误报「余额不足」（P0：防并发双扣的锁序不变，行锁仍先取）
    let balance: i64 =
        sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)")
            .bind(idem)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(false);
    if exists {
        return Ok(SpendOutcome::Replayed);
    }
    if balance < amount {
        return Err(DomainError::InsufficientSpark);
    }
    sqlx::query(
        "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \
         VALUES (nextval('spark_ledger_id_seq'), $1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(user_id)
    .bind(-amount)
    .bind(kind)
    .bind(ref_type)
    .bind(ref_id)
    .bind(idem)
    .bind(balance - amount)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("UPDATE users SET spark_balance = spark_balance - $2 WHERE id = $1")
        .bind(user_id)
        .bind(amount)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(SpendOutcome::Spent)
}

/// 入账（签到/利息/奖励）
pub async fn earn_spark(
    db: &PgPool,
    user_id: i64,
    amount: i64,
    kind: &str,
    idem: &str,
) -> DomainResult<()> {
    let mut tx = db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let balance: i64 =
        sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等检查必须在行锁之后（P0：防并发双入账）
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)")
            .bind(idem)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(false);
    if exists {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key, balance_after) \
         VALUES (nextval('spark_ledger_id_seq'), $1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(amount)
    .bind(kind)
    .bind(idem)
    .bind(balance + amount)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1")
        .bind(user_id)
        .bind(amount)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

// ============ 商店（M11） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct ShopItem {
    id: i64,
    name: String,
    kind: String,
    price: i64,
}

#[get("/shop/items")]
async fn shop_items(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let items = sqlx::query_as::<_, ShopItem>(
        "SELECT id, name, kind, price FROM shop_items WHERE active = true ORDER BY price",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(items))
}

#[derive(Deserialize)]
struct BuyReq {
    item_id: i64,
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/shop/buy")]
async fn shop_buy(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BuyReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let item: Option<(String, String, i64, serde_json::Value)> = sqlx::query_as(
        "SELECT name, kind, price, config FROM shop_items WHERE id = $1 AND active = true",
    )
    .bind(body.item_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((name, kind, price, config)) = item else {
        return Err(DomainError::NotFound(body.item_id));
    };

    // 幂等键必填（P1）：网络层重试必须携带同一键，否则双扣款
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.is_empty())
        .ok_or(DomainError::Validation("缺少 idempotency_key".into()))?;
    let outcome = spend_spark(
        &state.repo.db,
        auth.id,
        price,
        "shop",
        &idem,
        "shop_item",
        body.item_id,
    )
    .await?;

    // 订单落库（幂等键唯一）
    sqlx::query(
        "INSERT INTO shop_orders (user_id, item_id, price, idempotency_key, config_snapshot) \
         VALUES ($1, $2, $3, $4, $5) ON CONFLICT (idempotency_key) DO NOTHING",
    )
    .bind(auth.id)
    .bind(body.item_id)
    .bind(price)
    .bind(&idem)
    .bind(config.clone())
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 商品效果只在真实扣款时执行一次（幂等重放不重复发效果）
    if matches!(outcome, crate::economy_http::SpendOutcome::Spent) {
        let mut cfg = config.clone();
        cfg["item_id"] = serde_json::json!(body.item_id);
        apply_item_effect(&state.repo.db, auth.id, &kind, &cfg).await?;
    }

    state
        .repo
        .audit(Some(auth.id), "shop_buy", Some(body.item_id))
        .await;
    Ok(ok(
        serde_json::json!({ "item": name, "price": price, "idempotency_key": idem }),
    ))
}

async fn apply_item_effect(
    db: &PgPool,
    user_id: i64,
    kind: &str,
    config: &serde_json::Value,
) -> DomainResult<()> {
    match kind {
        // 上传量：等值正流量流水（§6.2 快照刷新由 worker 聚合，这里直接加账）
        "upload_credit" => {
            let gb = config.get("gb").and_then(|v| v.as_i64()).unwrap_or(0);
            sqlx::query(
                "INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, delta_down, window_start) \
                 VALUES (nextval('traffic_ledger_id_seq'), $1, 0, $2, 0, now())",
            )
            .bind(user_id)
            .bind(gb * 1024 * 1024 * 1024)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query("UPDATE users SET uploaded = uploaded + $2 WHERE id = $1")
                .bind(user_id)
                .bind(gb * 1024 * 1024 * 1024)
                .execute(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // 装扮（M25）：写入拥有记录（佩戴需显式调 /dressup/wear）
        "avatar_frame" | "animated_avatar" | "rainbow_id" | "rainbow_name" => {
            let item_id = config.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0);
            sqlx::query(
                "INSERT INTO user_dressups (user_id, item_id, source) VALUES ($1, $2, 'buy')                  ON CONFLICT (user_id, item_id) DO NOTHING",
            )
            .bind(user_id)
            .bind(item_id)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // 邀请：直接发一枚 72h 有效邀请码
        "invite" => {
            let code = crate::domain::new_invite_code();
            sqlx::query("INSERT INTO invites (inviter_id, code, expires_at) VALUES ($1, $2, now() + interval '72 hours')")
                .bind(user_id)
                .bind(&code)
                .execute(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
        _ => {} // 其余类型：权益标记后续按需扩展（佩戴/生效周期）
    }
    Ok(())
}

// ============ 我的火花（M11） ============

#[get("/me/spark")]
async fn my_spark(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let balance: i64 = sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
    // 收益因子说明（设计稿：1.03x/5x/0.1x 可点击展开）—— 由做种状态推导
    let seeding_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM snatches WHERE user_id = $1 AND seeding")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "balance": balance,
        "seeding_count": seeding_count,
        "hourly_estimate": 10 + seeding_count * 2,
    })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct LedgerRow {
    amount: i64,
    kind: String,
    balance_after: Option<i64>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct LedgerQuery {
    limit: Option<i64>,
}

#[get("/me/spark/ledger")]
async fn my_ledger(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<LedgerQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, LedgerRow>(
        "SELECT amount, kind, balance_after, created_at FROM spark_ledger \
         WHERE user_id = $1 ORDER BY id DESC LIMIT $2",
    )
    .bind(auth.id)
    .bind(q.limit.unwrap_or(20).clamp(1, 50))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ 银行（M11） ============

#[derive(Deserialize)]
struct DepositReq {
    amount: i64,
    term_days: i32,
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
            "定期存款单笔不少于 {} 火花",
            bs.min_deposit
        )));
    }
    if bs.max_deposit > 0 && body.amount > bs.max_deposit {
        return Err(DomainError::Validation(format!(
            "定期存款单笔不可超过 {} 火花",
            bs.max_deposit
        )));
    }
    let idem = format!("deposit:{}:{}", auth.id, Uuid::new_v4());
    spend_spark(
        &state.repo.db,
        auth.id,
        body.amount,
        "bank_deposit",
        &idem,
        "bank",
        0,
    )
    .await?;

    let interest = maturity_interest(body.amount, body.term_days);
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO bank_deposits (user_id, amount, term_days, rate, interest, maturity_at) \
         VALUES ($1, $2, $3, $4, $5, now() + ($3 || ' days')::interval) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.amount)
    .bind(body.term_days)
    .bind(term_rate(body.term_days))
    .bind(interest)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    Ok(ok(serde_json::json!({
        "id": id, "amount": body.amount, "term_days": body.term_days,
        "interest": interest, "rate": term_rate(body.term_days),
    })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct DepositRow {
    id: i64,
    amount: i64,
    term_days: i32,
    interest: i64,
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
        "SELECT id, amount, term_days, interest, status, maturity_at FROM bank_deposits \
         WHERE user_id = $1 ORDER BY id DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct WithdrawReq {
    deposit_id: i64,
}

#[post("/bank/withdraw")]
async fn bank_withdraw(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WithdrawReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let d: Option<(i64, i64, i64, i16, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT id, amount, interest, status, maturity_at FROM bank_deposits WHERE id = $1 AND user_id = $2",
    )
    .bind(body.deposit_id)
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((id, amount, interest, status, maturity_at)) = d else {
        return Err(DomainError::NotFound(body.deposit_id));
    };
    if status != 0 {
        return Err(DomainError::Validation("该存款已处理".into()));
    }
    let bs = bank_settings(&state.repo.db).await;
    let matured = chrono::Utc::now() >= maturity_at;
    // 到期：本息全额；提前支取：本金扣手续费，不计息（旧站口径）
    let (payable, penalty) = if matured {
        (amount + interest, 0i64)
    } else {
        let p = early_penalty(amount, bs.penalty_bp);
        (amount - p, p)
    };

    let updated = sqlx::query(
        "UPDATE bank_deposits SET status = 1, settled_at = now(), penalty = $2, withdrawn_at = now() \
         WHERE id = $1 AND status = 0",
    )
    .bind(id)
    .bind(penalty)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::LedgerConflict);
    }
    let idem = format!("withdraw:{}", id);
    earn_spark(&state.repo.db, auth.id, payable, "bank_withdraw", &idem).await?;
    Ok(ok(
        serde_json::json!({ "paid": payable, "matured": matured, "penalty": penalty,
            "interest_earned": if matured { interest } else { 0 } }),
    ))
}

// ============ 活期与贷款（0048 火花银行对齐） ============

#[derive(Deserialize)]
struct DemandDepositReq {
    amount: i64,
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
            "活期单笔存入不少于 {} 火花",
            bs.min_demand
        )));
    }
    let idem = format!("demand_in:{}:{}", auth.id, Uuid::new_v4());
    spend_spark(
        &state.repo.db,
        auth.id,
        body.amount,
        "bank_demand_in",
        &idem,
        "bank",
        0,
    )
    .await?;
    sqlx::query(
        "INSERT INTO bank_demand_accounts (user_id, balance, daily_rate_bp, last_interest_date) \
         VALUES ($1, $2, $3, CURRENT_DATE) \
         ON CONFLICT (user_id) DO UPDATE SET balance = bank_demand_accounts.balance + $2, \
         daily_rate_bp = $3, updated_at = now()",
    )
    .bind(auth.id)
    .bind(body.amount)
    .bind(bs.demand_rate_bp)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "deposited": body.amount })))
}

#[derive(Deserialize)]
struct DemandWithdrawReq {
    amount: i64,
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
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
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
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let idem = format!("demand_out:{}:{}", auth.id, Uuid::new_v4());
    earn_spark(&state.repo.db, auth.id, body.amount, "bank_demand_out", &idem).await?;
    Ok(ok(serde_json::json!({ "paid": body.amount, "balance_left": row })))
}

#[derive(Deserialize)]
struct LoanApplyReq {
    amount: i64,
    term_days: i32,
}

/// 最大可贷额度 = 时魔/小时 × 系数 + 常数（时魔取自近 1 小时做种收益口径，无则 0）
async fn max_loan_amount(db: &PgPool, user_id: i64, bs: &BankSettings) -> DomainResult<i64> {
    let hourly: Option<i64> = sqlx::query_scalar(
        "SELECT COALESCE(sum(amount), 0) FROM spark_ledger \
         WHERE user_id = $1 AND kind = 'seeding_reward' AND created_at > now() - interval '1 hour'",
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(hourly.unwrap_or(0) * bs.loan_ratio + bs.loan_constant)
}

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
            "贷款金额不少于 {} 火花",
            bs.min_loan
        )));
    }
    let balance: i64 = sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if balance < 0 {
        return Err(DomainError::Validation("当前火花为负，暂不可申请贷款".into()));
    }
    let max = max_loan_amount(&state.repo.db, auth.id, &bs).await?;
    if body.amount > max {
        return Err(DomainError::Validation(format!(
            "贷款金额不可超过额度上限 {} 火花（时魔 × {} + {}）",
            max, bs.loan_ratio, bs.loan_constant
        )));
    }
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM bank_loans WHERE user_id = $1 AND status = 'active')",
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
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO bank_loans (user_id, amount, daily_rate_bp, penalty_rate_bp, term_days, remaining, due_at, last_interest_date) \
         VALUES ($1, $2, $3, $4, $5, $2, now() + ($5 || ' days')::interval, CURRENT_DATE) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.amount)
    .bind(rate)
    .bind(bs.overdue_penalty_bp)
    .bind(body.term_days)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let idem = format!("loan:{}:{}", auth.id, id);
    earn_spark(&state.repo.db, auth.id, body.amount, "bank_loan_payout", &idem).await?;
    Ok(ok(serde_json::json!({
        "id": id, "amount": body.amount, "term_days": body.term_days, "daily_rate_bp": rate,
        "due_in_days": body.term_days,
    })))
}

#[post("/bank/loan/repay")]
async fn loan_repay(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 结清额 = 剩余本金 + 计提至今利息（含当日，一次性结清）
    let loan: Option<(i64, i64, i64, i64, chrono::NaiveDate)> = sqlx::query_as(
        "SELECT id, remaining, accrued_interest, daily_rate_bp, last_interest_date \
         FROM bank_loans WHERE user_id = $1 AND status = 'active' FOR UPDATE",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((id, remaining, accrued, rate_bp, last_date)) = loan else {
        return Err(DomainError::Validation("没有进行中的贷款".into()));
    };
    let days = (chrono::Utc::now().date_naive() - last_date).num_days();
    let today_interest = if days > 0 {
        economy::loan_interest(remaining, rate_bp as i32, days)
    } else {
        0
    };
    let payoff = remaining + accrued + today_interest;
    let idem = format!("loan_repay:{}", id);
    spend_spark(
        &state.repo.db,
        auth.id,
        payoff,
        "bank_loan_repay",
        &idem,
        "bank",
        id,
    )
    .await?;
    sqlx::query(
        "UPDATE bank_loans SET remaining = 0, accrued_interest = 0, status = 'paid', \
         paid_at = now(), last_interest_date = CURRENT_DATE WHERE id = $1 AND status = 'active'",
    )
    .bind(id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "paid": payoff, "principal": remaining,
        "interest": accrued + today_interest,
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

/// 银行总览：活期账户 + 资产汇总 + 当前贷款 + 额度
#[get("/bank/overview")]
async fn bank_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let bs = bank_settings(&state.repo.db).await;

    let demand: DemandRow = sqlx::query_as(
        "SELECT balance, daily_rate_bp, last_interest_date FROM bank_demand_accounts WHERE user_id = $1",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or(DemandRow { balance: 0, daily_rate_bp: bs.demand_rate_bp, last_interest_date: None });

    let fixed: Option<(i64, i64)> = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COALESCE(sum(amount), 0), count(*) FROM bank_deposits WHERE user_id = $1 AND status = 0",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let loan: Option<BankLoanRow> = sqlx::query_as(
        "SELECT id, amount, daily_rate_bp, term_days, remaining, accrued_interest, status, due_at \
         FROM bank_loans WHERE user_id = $1 AND status = 'active'",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let spark: i64 = sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    let loan_outstanding = loan.as_ref().map(|l| l.remaining + l.accrued_interest).unwrap_or(0);
    let total_asset = spark + demand.balance + fixed.unwrap_or((0, 0)).0;
    let max_loan = max_loan_amount(&state.repo.db, auth.id, &bs).await?;

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
        "fixed_rates": VALID_TERMS.iter().map(|t| serde_json::json!({
            "term_days": t, "annual_rate": term_rate(*t),
        })).collect::<Vec<_>>(),
        "loan_rates": LOAN_TERMS.iter().map(|t| serde_json::json!({
            "term_days": t, "daily_rate_bp": loan_rate_bp(*t),
        })).collect::<Vec<_>>(),
    })))
}

// ============ 签到（M12） ============

#[post("/attendance/checkin")]
async fn checkin(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive(); // 站点时区 UTC+8
    let yesterday = today - chrono::Duration::days(1);

    // 幂等：今日已签直接返回
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM attendance WHERE user_id = $1 AND date = $2)",
    )
    .bind(auth.id)
    .bind(today)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if already {
        return Err(DomainError::Validation("今天已经签到过啦".into()));
    }

    let last: Option<(chrono::NaiveDate, i32, i64)> = sqlx::query_as(
        "SELECT date, streak, (count(*) OVER ())::bigint FROM attendance WHERE user_id = $1 ORDER BY date DESC LIMIT 1",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let (prev_date, prev_streak, total_days): (Option<chrono::NaiveDate>, i64, i64) = match last {
        Some((d, s, c)) => (Some(d), s as i64, c),
        None => (None, 0, 0),
    };
    let streak = if prev_date == Some(yesterday) {
        prev_streak + 1
    } else {
        1
    };
    let reward = checkin_reward(streak, total_days == 0);

    let inserted = sqlx::query(
        "INSERT INTO attendance (user_id, date, streak, reward) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (user_id, date) DO NOTHING",
    )
    .bind(auth.id)
    .bind(today)
    .bind(streak)
    .bind(reward.total)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if inserted.rows_affected() == 0 {
        return Err(DomainError::Validation("今天已经签到过啦".into()));
    }

    let idem = format!("attendance:{}:{}", auth.id, today.format("%Y%m%d"));
    earn_spark(&state.repo.db, auth.id, reward.total, "attendance", &idem).await?;

    Ok(ok(serde_json::json!({
        "streak": reward.streak, "reward": reward.total,
        "base": reward.base, "streak_bonus": reward.streak_bonus,
    })))
}

#[get("/attendance")]
async fn checkin_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<(chrono::NaiveDate, i32, i64)> = sqlx::query_as(
        "SELECT date, streak, reward FROM attendance WHERE user_id = $1 AND date >= current_date - interval '30 days' ORDER BY date",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive(); // 站点时区 UTC+8
    let checked_today = rows.iter().any(|(d, _, _)| *d == today);
    let current_streak = rows.last().map(|(_, s, _)| *s).unwrap_or(0);
    Ok(ok(serde_json::json!({
        "checked_today": checked_today, "streak": current_streak,
        "recent": rows,
    })))
}

// ============ 站免池（M13） ============

#[get("/magic-pool")]
async fn pool_status(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let month = economy::pool_month(chrono::Utc::now());
    let row: Option<(String, i64, i64, bool)> = sqlx::query_as(
        "SELECT month, donated_total, goal, promo_started FROM magic_pool WHERE month = $1",
    )
    .bind(&month)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (m, donated, goal, started) =
        row.unwrap_or((month.clone(), 0, economy::MAGIC_POOL_GOAL, false));
    let top: Vec<(String, i64)> = sqlx::query_as(
        "SELECT u.username, sum(d.amount)::bigint FROM pool_donations d \
         JOIN users u ON u.id = d.user_id WHERE d.month = $1 \
         GROUP BY u.username ORDER BY 2 DESC LIMIT 10",
    )
    .bind(&month)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "month": m, "donated": donated, "goal": goal, "progress": if goal > 0 { donated as f64 / goal as f64 } else { 0.0 },
        "promo_started": started, "top_donors": top,
    })))
}

#[derive(Deserialize)]
struct DonateReq {
    amount: i64,
}

#[post("/magic-pool/donate")]
async fn pool_donate(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DonateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.amount <= 0 {
        return Err(DomainError::Validation("捐赠金额必须为正".into()));
    }
    let month = economy::pool_month(chrono::Utc::now());
    let idem = format!("donate:{}:{}:{}", auth.id, month, Uuid::new_v4());
    spend_spark(
        &state.repo.db,
        auth.id,
        body.amount,
        "pool_donate",
        &idem,
        "pool",
        0,
    )
    .await?;

    sqlx::query(
        "INSERT INTO magic_pool (month, donated_total) VALUES ($1, $2) \
         ON CONFLICT (month) DO UPDATE SET donated_total = magic_pool.donated_total + $2",
    )
    .bind(&month)
    .bind(body.amount)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("INSERT INTO pool_donations (user_id, amount, month) VALUES ($1, $2, $3)")
        .bind(auth.id)
        .bind(body.amount)
        .bind(&month)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "month": month, "donated": body.amount }),
    ))
}

// ============ M25 装扮中心 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct DressupRow {
    item_id: i64,
    name: String,
    kind: String,
    price: i64,
    slot: Option<String>,
    owned: bool,
    wearing: bool,
}

/// 装扮列表（拥有状态 + 佩戴中）
#[get("/dressup/list")]
async fn dressup_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<DressupRow> = sqlx::query_as(
        "SELECT si.id AS item_id, si.name, si.kind, si.price,             si.config->>'slot' AS slot,             EXISTS(SELECT 1 FROM user_dressups ud WHERE ud.user_id = $1 AND ud.item_id = si.id) AS owned,             COALESCE((SELECT ud.wearing FROM user_dressups ud WHERE ud.user_id = $1 AND ud.item_id = si.id), FALSE) AS wearing          FROM shop_items si          WHERE si.active AND si.kind IN ('avatar_frame','animated_avatar','rainbow_id','rainbow_name')          ORDER BY si.price",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct WearReq {
    item_id: i64,
    wear: bool,
}

/// 佩戴/摘下（同类互斥由 DB 触发器保证：佩戴前先摘同类）
#[post("/dressup/wear")]
async fn dressup_wear(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WearReq>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let owned: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT si.config->>'slot' FROM user_dressups ud JOIN shop_items si ON si.id = ud.item_id          WHERE ud.user_id = $1 AND ud.item_id = $2",
    )
    .bind(auth.id)
    .bind(body.item_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((slot,)) = owned else {
        return Err(DomainError::Validation(
            "尚未拥有该装扮（先在商店购买）".into(),
        ));
    };

    if body.wear {
        // 先摘下同槽位（触发器互斥的最简前置）。注意 UPDATE...FROM 里目标表列不能带别名前缀
        sqlx::query(
            r#"UPDATE user_dressups ud SET wearing = FALSE FROM shop_items si
             WHERE si.id = ud.item_id AND ud.user_id = $1 AND si.config->>'slot' = $2"#,
        )
        .bind(auth.id)
        .bind(&slot)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        sqlx::query("UPDATE user_dressups SET wearing = TRUE WHERE user_id = $1 AND item_id = $2")
            .bind(auth.id)
            .bind(body.item_id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    } else {
        sqlx::query("UPDATE user_dressups SET wearing = FALSE WHERE user_id = $1 AND item_id = $2")
            .bind(auth.id)
            .bind(body.item_id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    state
        .repo
        .audit(
            Some(auth.id),
            if body.wear {
                "dressup.wear"
            } else {
                "dressup.takeoff"
            },
            Some(body.item_id),
        )
        .await;
    Ok(ok(
        serde_json::json!({ "item_id": body.item_id, "wearing": body.wear }),
    ))
}
