//! M14 勋章购买与赠送。
//! 从 community_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use uuid::Uuid;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
struct MedalBuyReq {
    medal_id: i64,
    /// 前端生成的幂等键（同一键重试不双扣）；缺省时回退随机键（兼容旧客户端）
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/medals/buy")]
async fn medal_buy(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MedalBuyReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let price: Option<i64> =
        sqlx::query_scalar("SELECT price FROM medals WHERE id = $1")
            .bind(body.medal_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
    let Some(price) = price else {
        return Err(DomainError::NotFound(body.medal_id)); // 非卖品勋章（如开站勋章）
    };
    // 已拥有直接拒绝（防重复扣款）
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_medals WHERE user_id = $1 AND medal_id = $2 AND (expires_at IS NULL OR expires_at > now()))",
    )
    .bind(auth.id)
    .bind(body.medal_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if owned {
        return Err(DomainError::Validation("已拥有该勋章".into()));
    }
    // 限量与销售期校验（medals.inventory NULL = 不限量；窗口 NULL = 长期在售）
    let (inventory, inv_used, sale_begin, sale_end): (
        Option<i32>,
        i64,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        "SELECT m.inventory, (SELECT count(*) FROM user_medals um WHERE um.medal_id = m.id), \
                m.sale_begin_at, m.sale_end_at \
         FROM medals m WHERE m.id = $1",
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
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "medal-buy:{}:{}:{}",
                auth.id,
                body.medal_id,
                Uuid::new_v4()
            )
        });
    // 幂等重放闸门（#[must_use] 连审）：重放时 spend 不再扣款，继续执行会绕过
    // 上方的拥有/售期/限量三重检查直接走授予分支
    if !matches!(
        crate::economy_http::spend_spark(
            &state.repo.db,
            auth.id,
            price,
            "shop",
            &idem,
            "medal",
            body.medal_id,
        )
        .await?,
        crate::economy_http::SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "该笔请求已受理，请勿重复提交".into(),
        ));
    }
    // 限时勋章按 duration_days 写 expires_at（0067；NULL = 永久）
    sqlx::query(
        "INSERT INTO user_medals (user_id, medal_id, source, expires_at) \
         SELECT $1, $2, 'buy', now() + make_interval(days => m.duration_days) \
         FROM medals m WHERE m.id = $2 \
         ON CONFLICT (user_id, medal_id) DO NOTHING",
    )
    .bind(auth.id)
    .bind(body.medal_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "medal_id": body.medal_id, "price": price }),
    ))
}
