//! M14 勋章赠送（medal_gift）。
//! 从 community_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use uuid::Uuid;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
struct MedalGiftReq {
    medal_id: i64,
    to_user: String,
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/medals/gift")]
async fn medal_gift(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MedalGiftReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let to_id: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE username = $1 AND status < 2",
    )
    .bind(&body.to_user)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .flatten();
    let Some(to_id) = to_id else {
        return Err(DomainError::NotFound(0));
    };
    if to_id == auth.id {
        return Err(DomainError::Validation("不能赠送给自己".into()));
    }
    // 购买并直接入对方账户（赠送弹窗流程：一步完成）
    let price: Option<i64> =
        sqlx::query_scalar("SELECT price FROM medals WHERE id = $1")
            .bind(body.medal_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
    let Some(price) = price else {
        return Err(DomainError::NotFound(body.medal_id));
    };
    // 赠送税（0078）：礼物链路抽 gift_tax_bp（缺省 5%）入站免池——
    // 扣款仍按全额（spend_spark price），勋章照常发放；税在「站点收入」侧记账，
    // 即 magic_pool/pool_donations（出资人=送礼人），不另记正向流水（防虚增 minted）。
    let tax_bp: i32 = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'gift_tax_bp'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .and_then(|v: String| v.parse().ok())
    .unwrap_or(500);
    let tax = crate::economy::gift_tax(price, tax_bp);
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "medal-gift:{}:{}:{}",
                auth.id,
                body.medal_id,
                Uuid::new_v4()
            )
        });
    // 赠送通道同样受「已拥有/售期/限量」约束（此前 gift 绕过三重检查可超卖限量勋章）
    let receiver_owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_medals WHERE user_id = $1 AND medal_id = $2 AND (expires_at IS NULL OR expires_at > now()))",
    )
    .bind(to_id)
    .bind(body.medal_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if receiver_owned {
        return Err(DomainError::Validation("对方已拥有该勋章".into()));
    }
    let (inventory, inv_used, sale_begin, sale_end): (
        Option<i32>,
        i64,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        "SELECT m.inventory, (SELECT count(*) FROM user_medals um WHERE um.medal_id = m.id),                 m.sale_begin_at, m.sale_end_at          FROM medals m WHERE m.id = $1",
    )
    .bind(body.medal_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(begin) = sale_begin {
        if chrono::Utc::now() < begin {
            return Err(DomainError::Validation("该勋章尚未开售".into()));
        }
    }
    if let Some(end) = sale_end {
        if chrono::Utc::now() > end {
            return Err(DomainError::Validation("该勋章已结束销售".into()));
        }
    }
    if let Some(stock) = inventory {
        if inv_used >= stock as i64 {
            return Err(DomainError::Validation("该勋章已售罄".into()));
        }
    }
    // 幂等重放闸门（P0）：赠送链路是 spend(赠送人) → earn(受赠人) 对冲，重放时
    // spend 不再扣款而 earn 照发 = 受赠人凭空入账
    if !matches!(
        crate::economy_http::spend_spark(
            &state.repo.db,
            auth.id,
            price,
            "shop",
            &idem,
            "medal_gift",
            body.medal_id,
        )
        .await?,
        crate::economy_http::SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "该笔赠送已受理，请勿重复提交".into(),
        ));
    }
    if tax > 0 {
        let month = crate::economy::pool_month(chrono::Utc::now());
        sqlx::query(
            "INSERT INTO magic_pool (month, donated_total) VALUES ($1, $2) \
             ON CONFLICT (month) DO UPDATE SET donated_total = magic_pool.donated_total + $2",
        )
        .bind(&month)
        .bind(tax)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        sqlx::query("INSERT INTO pool_donations (user_id, amount, month) VALUES ($1, $2, $3)")
            .bind(auth.id)
            .bind(tax)
            .bind(&month)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    sqlx::query(
        "INSERT INTO user_medals (user_id, medal_id, source, expires_at) \
         SELECT $1, $2, 'gift', now() + make_interval(days => m.duration_days) \
         FROM medals m WHERE m.id = $2 \
         ON CONFLICT (user_id, medal_id) DO NOTHING",
    )
    .bind(to_id)
    .bind(body.medal_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 收件人通知（审计补齐：收礼物却无感知，只能自己去勋章页发现）
    let _ = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES ($1, $2, $3, $4)",
    )
    .bind(auth.id)
    .bind(to_id)
    .bind("收到勋章礼物")
    .bind(format!(
        "用户 #{} 向你赠送了勋章「#{}」，快去勋章页看看吧！",
        auth.id, body.medal_id
    ))
    .execute(&state.repo.db)
    .await;
    Ok(ok(
        serde_json::json!({ "to": body.to_user, "medal_id": body.medal_id }),
    ))
}
