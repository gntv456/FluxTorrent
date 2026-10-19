//! M14 勋章购买与赠送。
//! 从 community_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use uuid::Uuid;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize, utoipa::ToSchema)]
struct MedalBuyReq {
    medal_id: i64,
    /// 前端生成的幂等键（同一键重试不双扣）；缺省时回退随机键（兼容旧客户端）
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/v1/medals/buy",
    tag = "社区/勋章",
    description = "用魔力购买勋章。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[post("/medals/buy")]
pub(crate) async fn medal_buy(
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
        "SELECT EXISTS(SELECT 1 FROM user_medals WHERE user_id = $1 \
         AND medal_id = $2 AND (expires_at IS NULL OR expires_at > now()))",
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
    // 体字段优先、Idempotency-Key 头兜底（缺省回退随机键链保持不变）
    let idem = crate::http::idempotency::body_or_header(&body.idempotency_key)
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
    // 限时勋章按 duration_days 写 expires_at（0067；NULL = 永久）。
    // P-350（与赠送口 P-345 同形）：上面的「已拥有」判据只看未过期的行，
    // 而 user_medals 主键是 (user_id, medal_id) —— 过期件重买时旧写法写 0 行，
    // 扣款与回显都成功、勋章却没复活。这里过期件按产品口径复活重算有效期；
    // 仍为 0 行只可能是并发竞取，退款并报中文错误，绝不收钱不发货。
    let granted: u64 = sqlx::query(
        "INSERT INTO user_medals (user_id, medal_id, source, expires_at) \
         SELECT $1, $2, 'buy', now() + make_interval(days => m.duration_days) \
         FROM medals m WHERE m.id = $2 \
         ON CONFLICT (user_id, medal_id) DO UPDATE SET source = 'buy', \
           expires_at = EXCLUDED.expires_at \
         WHERE user_medals.expires_at IS NOT NULL \
           AND user_medals.expires_at <= now()",
    )
    .bind(auth.id)
    .bind(body.medal_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if granted == 0 {
        let refund = format!("medal-buy-refund:{idem}");
        crate::economy_http::earn_spark(
            &state.repo.db,
            auth.id,
            price,
            "shop",
            &refund,
        )
        .await?;
        return Err(DomainError::Validation(
            "勋章发放失败，本次扣款已退回，请稍后重试".into(),
        ));
    }
    Ok(ok(
        serde_json::json!({ "medal_id": body.medal_id, "price": price }),
    ))
}
