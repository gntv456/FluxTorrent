//! 捐赠回调与订单管理。
//! 从 staff_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

/// 支付网关异步回调（GET，易支付口径）：验签 → 幂等入账 → 纯文本 "success"
/// （网关要求响应 success 字面量，不走统一信封）
#[get("/donate/notify")]
pub async fn donate_notify(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let _ = req;
    match crate::payment::settle_notify(&state, &q.into_inner()).await {
        Ok(_) => HttpResponse::Ok().body("success"),
        Err(e) => {
            tracing::warn!(?e, "donate notify rejected");
            HttpResponse::Ok().body("fail")
        }
    }
}

/// 旧模拟充值逻辑（FLUX_DEMO=1 专用）：移入 payment.rs 兄弟函数，保留演示站能力
#[allow(dead_code)]
mod removed_legacy_simulated_topup {
    // 历史实现已由 payment::demo_topup 替代（编译期占位，防误回滚参考）
}

#[derive(Deserialize)]
struct OrderBody {
    plan_id: i32,
}

/// 用余额订购套餐（上传量 / 邀请名额 / VIP）
#[post("/donate/order")]
pub async fn donate_order(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<OrderBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let plan: Option<(String, String, Option<String>, f64)> = sqlx::query_as(
        "SELECT plan_type, title, reward, \
         price_usd::float8 FROM donation_plans WHERE id = $1 AND enabled",
    )
    .bind(body.plan_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((plan_type, title, reward, price)) = plan else {
        return Err(DomainError::NotFound(body.plan_id as i64));
    };
    // 发放量取自标题首段数字（upload 计 GB、quota 计枚），上限沿用批量发放口径
    // （上传 10 TB / 邀请 50 枚）。必须在扣款之前判定：解析不出就拒单，
    // 收钱不给货是这条链路上唯一不可逆的损失。
    let cap: i64 = match plan_type.as_str() {
        "upload" => 10_000,
        "quota" => 50,
        _ => 0,
    };
    let grant: i64 = title
        .split_whitespace()
        .next()
        .and_then(|w| w.parse().ok())
        .filter(|v| (1..=cap).contains(v))
        .unwrap_or(0);
    if cap > 0 && grant == 0 {
        // 校验详情必须整句可查 validation_details.tsv，所以不能用 format! 拼
        // （validation_i18n_guard 锁动态串条数）；标题与上限只进日志。
        tracing::warn!(
            plan_id = body.plan_id,
            title = %title,
            cap,
            "donate order rejected: plan title head number invalid"
        );
        return Err(DomainError::Validation(
            "套餐标题首段数量无效，已拒单未扣款".into(),
        ));
    }
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let balance: Option<f64> = sqlx::query_scalar(
        "UPDATE users SET wallet_usd = wallet_usd - $2 WHERE id = $1 \
         AND wallet_usd >= $2 RETURNING wallet_usd::float8",
    )
    .bind(auth.id)
    .bind(price)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(balance) = balance else {
        return Err(DomainError::Validation(format!(
            "余额不足，还差 {:.2} USD",
            price
        )));
    };
    // 套餐生效
    match plan_type.as_str() {
        "upload" => {
            // 「100 GB 上传量」/「500 GB 上传量」→ grant 已是 GB 数
            // 必须落差额流水（P1）：快照权威在 traffic_ledger，reconcile_snapshots 每 6h
            // 把 users.uploaded 重算为 sum(delta_up)——只 UPDATE 快照不落流水，付费购买的
            // 上传量会在 6h 内被静默抹掉。对照 shop upload_credit 的双写。
            sqlx::query(
                "INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, delta_down, window_start) \
                 VALUES (nextval('traffic_ledger_id_seq'), $1, 0, $2, 0, now())",
            )
            .bind(auth.id)
            .bind(grant * 1024 * 1024 * 1024)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query(
                "UPDATE users SET uploaded = uploaded + $2 WHERE id = $1",
            )
            .bind(auth.id)
            .bind(grant * 1024 * 1024 * 1024)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        "quota" => {
            // 「10 枚邀请名额」→ grant 已是枚数；领码时 quota_extra 优先于周配额消耗
            sqlx::query(
                "UPDATE users SET quota_extra = quota_extra + $2 WHERE id = $1",
            )
            .bind(auth.id)
            .bind(grant as i32)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        "vip" => {
            let days: i64 = if title.contains("终身") {
                36500
            } else if title.contains("180") {
                180
            } else {
                30
            };
            sqlx::query(
                "UPDATE users SET vip_until = \
                 GREATEST(COALESCE(vip_until, now()), now()) + \
                 make_interval(days => $2::int) WHERE id = $1",
            )
            .bind(auth.id)
            .bind(days)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        _ => {}
    }
    // 附赠邀请 1（invite_quota 是 (user_id, period) 结构：当天无行则插入 used=0）
    if reward.as_deref().unwrap_or("").contains("邀请") {
        sqlx::query(
            "INSERT INTO invite_quota (user_id, period, used) \
             VALUES ($1, current_date, 0) ON CONFLICT (user_id, period) DO \
             NOTHING",
        )
        .bind(auth.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    sqlx::query(
        "INSERT INTO donation_ledger (user_id, kind, amount_usd, \
         balance_after, plan_id, note) VALUES ($1, 'order', -$2, $3, $4, $5)",
    )
    .bind(auth.id)
    .bind(price)
    .bind(balance)
    .bind(body.plan_id)
    .bind(&title)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "donate_order", Some(body.plan_id as i64))
        .await;
    Ok(ok(
        serde_json::json!({ "plan": title, "wallet_usd": balance }),
    ))
}

// ---- 批量邮件（massmail）----
