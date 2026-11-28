//! 付费定价端点（2026-11-28 从 manage.rs 拆出守行数门禁）：
//! 发布者或 staff 设置付费价格；改价不影响已购（以 torrent_purchases
//! 已扣为准）。edit/restore/resubmit 编排仍在 manage.rs。
use actix_web::{put, web, HttpRequest, HttpResponse};
use serde_json::Value;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::aggregate::invalidate_tdetail_cache;

/// 修改付费定价（0086）：发布者或 staff；改价不影响已购（以 torrent_purchases 已扣为准）
#[derive(serde::Deserialize, utoipa::ToSchema)]
struct PriceReq {
    price: i64,
}

#[utoipa::path(
    put,
    path = "/api/v1/torrents/{id}/price",
    tag = "种子",
    description = "设置付费价格（发布者本人）。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
        (status = 422, description = "参数校验失败（code 1002）"),
        (status = 403, description = "权限不足（code 2003）"),
    )
)]
#[put("/torrents/{id}/price")]
pub(crate) async fn set_torrent_price(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<PriceReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let owner: Option<i64> =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner_id) = owner else {
        return Err(DomainError::NotFound(id));
    };
    if owner_id != auth.id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    // P-3：改价口与发种口同一闸门（paid_torrent_enabled / max_price 此前
    // 零读取方，站长关掉付费后作者仍可用本口把免费种改成付费种）。
    // 越界明确报错而非 clamp——与发种口「静默改写即缺陷」同口径。
    let gate =
        crate::publish_http::upload_precheck_pub::price_gate(&state.repo.db)
            .await;
    if body.price != 0 && !gate.enabled {
        return Err(DomainError::Validation(
            "本站未开启付费种子：价格只能为 0（免费）".into(),
        ));
    }
    if body.price > gate.limit {
        return Err(DomainError::Validation(format!(
            "付费下载价格需在 0–{} 之间（站点售价上限）",
            gate.limit
        )));
    }
    let price = body.price;
    sqlx::query("UPDATE torrents SET price = $2 WHERE id = $1")
        .bind(id)
        .bind(price)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "torrent.price_set", Some(id))
        .await;
    super::aggregate::invalidate_tdetail_cache(&state, id).await;
    Ok(ok(serde_json::json!({ "price": price })))
}
