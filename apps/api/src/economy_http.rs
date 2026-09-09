//! 经济系统 HTTP 接口（M11 商店/流水/银行 + M12 签到 + M13 站免池）。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::ok;
use crate::economy::{self, checkin_reward, maturity_interest, term_rate, VALID_TERMS};
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

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
        .service(checkin)
        .service(checkin_status)
        .service(pool_status)
        .service(pool_donate)
}

/// 动账核心：余额充足校验 + 负流水 + 余额快照更新（单事务）。
/// 幂等键唯一约束（shop_orders/应用层先查）防重复扣款。
pub async fn spend_spark(
    db: &PgPool,
    user_id: i64,
    amount: i64,
    kind: &str,
    idem: &str,
    ref_type: &str,
    ref_id: i64,
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
    if balance < amount {
        return Err(DomainError::InsufficientSpark);
    }
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)")
            .bind(idem)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(false);
    if exists {
        return Ok(()); // 幂等重放
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
    Ok(())
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

    let idem = body
        .idempotency_key
        .clone()
        .unwrap_or_else(|| format!("buy:{}:{}", auth.id, Uuid::new_v4()));
    spend_spark(
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

    // 商品效果（上传量立即到账；邀请发放）
    apply_item_effect(&state.repo.db, auth.id, &kind, &config).await?;

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
    let matured = chrono::Utc::now() >= maturity_at;
    let payable = if matured { amount + interest } else { amount }; // 提前支取不计息（旧站口径）

    let updated = sqlx::query(
        "UPDATE bank_deposits SET status = 1, settled_at = now() WHERE id = $1 AND status = 0",
    )
    .bind(id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::LedgerConflict);
    }
    let idem = format!("withdraw:{}", id);
    earn_spark(&state.repo.db, auth.id, payable, "bank_withdraw", &idem).await?;
    Ok(ok(
        serde_json::json!({ "paid": payable, "matured": matured, "interest_earned": if matured { interest } else { 0 } }),
    ))
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
        "SELECT date, streak, (count(*) OVER ())::int FROM attendance WHERE user_id = $1 ORDER BY date DESC LIMIT 1",
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
