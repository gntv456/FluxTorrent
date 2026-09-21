//! 捐赠状态/充值/订单查询。
//! 从 staff_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

/// 捐赠中心总览：钱包余额 + VIP 状态 + 套餐 + 我的流水
#[get("/donate/state")]
pub async fn donate_state(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let (wallet, vip_until): (f64, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as(
            "SELECT wallet_usd::float8, vip_until FROM users WHERE id = $1",
        )
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let plans: Vec<DonatePlan> = sqlx::query_as(
        "SELECT id, plan_type, title, reward, price_usd::float8, \
         sort FROM donation_plans WHERE enabled ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let ledger: Vec<DonateLedgerRow> = sqlx::query_as(
                "SELECT id, kind, amount_usd::float8, balance_after::float8, \
         note, \
         created_at FROM donation_ledger WHERE user_id = $1 ORDER BY id DESC LIMIT 30",
    ).bind(auth.id)
    .fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "wallet_usd": wallet,
        "vip_until": vip_until,
        "plans": plans,
        "ledger": ledger,
        // U4 §12.1：通道可用性（provider=epay 且凭证齐全；FLUX_DEMO 演示模式恒开）
        "payment_enabled": crate::payment::gateway_config(&state).await.available()
            || std::env::var("FLUX_DEMO").unwrap_or_default() == "1",
    })))
}

#[derive(Deserialize)]
struct TopupBody {
    amount_usd: f64,
    #[serde(default)]
    channel: String, // alipay / wechat（Dev 环境仅记录）
}

/// 充值（Dev 无支付网关：直接入账，模拟支付成功）
#[post("/donate/topup")]
pub async fn donate_topup(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TopupBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if !(10.0..=66.0).contains(&body.amount_usd) {
        return Err(DomainError::Validation("单笔 10 ~ 66 USD".into()));
    }
    if !["alipay", "wechat"].contains(&body.channel.as_str()) {
        return Err(DomainError::Validation(
            "支付方式需为 alipay/wechat".into(),
        ));
    }
    // U4 §12.1：真实订单流——按 site_settings 构造网关；未配置（none/缺凭证）拒单。
    // FLUX_DEMO=1 演示环境保留旧模拟充值（演示数据需要有钱包入账可看）。
    let gw = crate::payment::gateway_config(&state).await;
    if !gw.available() {
        if std::env::var("FLUX_DEMO").unwrap_or_default() == "1" {
            return crate::payment::demo_topup(
                &state,
                auth.id,
                body.amount_usd,
                &body.channel,
            )
            .await
            .map(|v| ok(v));
        }
        return Err(DomainError::Validation(
            "捐赠通道未开放（站长未配置支付网关）".into(),
        ));
    }
    // 建订单 + 返回跳转 URL（入账只发生在验签通过的回调，此端点不动钱包）
    let order_no = crate::payment::new_order_no(auth.id);
    sqlx::query(
        "INSERT INTO payment_orders (order_no, user_id, amount_usd, \
         channel) VALUES ($1, $2, $3, $4)",
    )
    .bind(&order_no)
    .bind(auth.id)
    .bind(body.amount_usd)
    .bind(&body.channel)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let provider = crate::payment::provider_from(&gw);
    let base = std::env::var("PUBLIC_SITE_URL")
        .unwrap_or_else(|_| "http://localhost:3000".into());
    let url = provider.pay_url(
        &order_no,
        &format!("{:.2}", body.amount_usd),
        &body.channel,
        "站点捐赠",
        &format!("{base}/donate?order={order_no}"),
        &format!("{base}/api/v1/donate/notify"),
    );
    state
        .repo
        .audit(Some(auth.id), "donate_topup_order", None)
        .await;
    Ok(ok(
        serde_json::json!({ "order_no": order_no, "pay_url": url }),
    ))
}

/// 捐赠订单状态查询（前端支付回跳后轮询）
#[get("/donate/order-status")]
pub async fn donate_order_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let Some(order_no) = q.get("order_no") else {
        return Err(DomainError::Validation("缺少 order_no".into()));
    };
    let st: Option<String> = sqlx::query_scalar(
        "SELECT status FROM payment_orders WHERE order_no = $1 AND \
         user_id = $2",
    )
    .bind(order_no)
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "order_no": order_no, "status": st }),
    ))
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub(super) struct DonatePlan {
    pub(super) id: i32,
    pub(super) plan_type: String,
    pub(super) title: String,
    pub(super) reward: Option<String>,
    #[sqlx(default)]
    pub(super) price_usd: f64,
    pub(super) sort: i32,
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub(super) struct DonateLedgerRow {
    pub(super) id: i64,
    pub(super) kind: String,
    #[sqlx(default)]
    pub(super) amount_usd: f64,
    #[sqlx(default)]
    pub(super) balance_after: f64,
    pub(super) note: Option<String>,
    pub(super) created_at: chrono::DateTime<chrono::Utc>,
}
