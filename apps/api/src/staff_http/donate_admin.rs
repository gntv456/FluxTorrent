//! 捐赠订单后台管理（0208 P1-6）：列表/筛选/手工补单。
//! 此前 payment_orders 只有用户自查与回调写入，网关掉单时站长只能进数据库——
//! 这里补上运营侧出口。补单语义与 settle_notify 的入账分支逐字对齐
//! （pending → paid + 钱包 + 流水，事务内幂等）。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(sqlx::FromRow, serde::Serialize)]
struct PaymentOrderRow {
    id: i64,
    order_no: String,
    user_id: i64,
    #[sqlx(default)]
    username: Option<String>,
    amount_usd: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    amount_paid: Option<f64>,
    channel: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trade_no: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    /// E7：档位已发放（回调/补单共用 CAS 置位）
    #[serde(default)]
    tier_granted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    paid_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Deserialize)]
struct OrderListQ {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    uid: Option<i64>,
    #[serde(default = "crate::admin_http::default_page")]
    page: i64,
    #[serde(default = "crate::admin_http::default_per_page")]
    per_page: i64,
}

/// 捐赠订单列表（0208 P1-6）：状态/用户筛选 + 分页 + 对账合计。
#[get("/admin/payment-orders")]
pub async fn admin_payment_orders(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<OrderListQ>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_ADJUST)
        .await?;
    if !(1..=100).contains(&q.per_page) {
        return Err(DomainError::Validation("per_page 取值 1-100".into()));
    }
    let status = q.status.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let rows: Vec<PaymentOrderRow> = sqlx::query_as(
        r#"SELECT o.id, o.order_no, o.user_id, u.username, o.amount_usd::float8,
                  o.amount_paid::float8, o.channel, o.status, o.trade_no, o.created_at, o.paid_at,
                  o.tier_granted
           FROM payment_orders o LEFT JOIN users u ON u.id = o.user_id
           WHERE ($1::text IS NULL OR o.status = $1)
             AND ($2::bigint IS NULL OR o.user_id = $2)
           ORDER BY o.id DESC LIMIT $3 OFFSET $4"#,
    )
    .bind(status)
    .bind(q.uid)
    .bind(crate::dto::page_window(q.page, q.per_page).1)
    .bind(crate::dto::page_window(q.page, q.per_page).0)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (total, paid_sum): (i64, f64) = sqlx::query_as(
        r#"SELECT count(*),
                  COALESCE(sum(amount_paid) FILTER (WHERE status = 'paid'), 0)::float8
           FROM payment_orders o
           WHERE ($1::text IS NULL OR o.status = $1)
             AND ($2::bigint IS NULL OR o.user_id = $2)"#,
    )
    .bind(status)
    .bind(q.uid)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "rows": rows, "total": total, "paid_sum_usd": paid_sum,
        "page": q.page.max(1), "per_page": q.per_page,
    })))
}

#[derive(Deserialize)]
struct ManualCompleteReq {
    order_no: String,
    /// 实收金额（缺省按订单面额）
    #[serde(default)]
    amount_paid: Option<f64>,
}

/// 手工补单（0208 P1-6）：网关掉单/线下收款核对后，把 pending 订单置为
/// paid 并入账（钱包 + donation_ledger + donor 标记），语义与回调一致；
/// 重复补单幂等返回 duplicate。金额以 max(实收, 0) 为准。
#[post("/admin/payment-orders/complete")]
pub async fn admin_payment_complete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ManualCompleteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_ADJUST)
        .await?;
    let order_no = body.order_no.trim();
    if order_no.is_empty() {
        return Err(DomainError::Validation("缺少 order_no".into()));
    }
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let claimed: Option<i64> = sqlx::query_scalar(
        "UPDATE payment_orders SET status='paid', \
         amount_paid = COALESCE($2, amount_usd), paid_at=now(), \
         trade_no = COALESCE(trade_no, $3) \
         WHERE order_no=$1 AND status='pending' RETURNING user_id",
    )
    .bind(order_no)
    .bind(body.amount_paid)
    .bind(format!("manual:{})", auth.id))
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(user_id) = claimed else {
        // 已 paid 的订单幂等返回；不存在/其它状态如实报
        let st: Option<String> = sqlx::query_scalar(
            "SELECT status FROM payment_orders WHERE order_no = $1",
        )
        .bind(order_no)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        return match st.as_deref() {
            Some("paid") => Ok(ok(serde_json::json!({
                "order_no": order_no, "result": "duplicate",
            }))),
            Some(s) => Err(DomainError::Validation(format!(
                "订单状态为 {s}，仅 pending 可补单"
            ))),
            None => Err(DomainError::NotFound(0)),
        };
    };
    let amount: f64 = sqlx::query_scalar(
        "SELECT amount_paid::float8 FROM payment_orders WHERE order_no = $1",
    )
    .bind(order_no)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let balance: f64 = sqlx::query_scalar(
        "UPDATE users SET wallet_usd = wallet_usd + $2, donor = true \
         WHERE id = $1 RETURNING wallet_usd::float8",
    )
    .bind(user_id)
    .bind(amount)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO donation_ledger \
         (user_id, kind, amount_usd, balance_after, note) \
         VALUES ($1, 'topup', $2, $3, $4)",
    )
    .bind(user_id)
    .bind(amount)
    .bind(balance)
    .bind(format!("管理端手工补单 order_no={order_no}"))
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // E7：补单同事务走档位发放（与回调同一 CAS 闸，双入口只发一次）
    let cum = crate::payment::cumulative_paid(&mut tx, user_id).await?;
    let granted =
        crate::payment::grant_tier(&mut tx, &state, order_no, user_id, cum)
            .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "admin.payment_complete", None)
        .await;
    if let Some(summary) = &granted {
        let email: Option<String> =
            sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_optional(&state.repo.db)
                .await
                .ok()
                .flatten();
        crate::mailer::notify_kind(
            &state.repo.db,
            user_id,
            "donate_tier",
            email,
            "捐赠回馈已发放",
            &format!("感谢支持！本次捐赠触发回馈档位 {summary}，已自动入账。"),
        )
        .await;
    }
    Ok(ok(serde_json::json!({
        "order_no": order_no, "result": "paid", "amount": amount,
        "tier": granted,
    })))
}
