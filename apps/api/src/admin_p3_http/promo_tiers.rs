//! 促销价目档 CRUD（0213）：promo_kind_tiers 的增改删。
//! 从 promo_kinds.rs 按域拆出（300 行门禁）。权限 SETTINGS_MANAGE。

use actix_web::{delete, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

#[derive(Deserialize)]
pub(super) struct TierReq {
    kind: String,
    hours: i32,
    price: i64,
    #[serde(default)]
    enabled: Option<bool>,
}

/// 新增/更新价目档（(kind, hours) 唯一，冲突即改价）
#[post("/admin/promo-kinds/tiers")]
pub(super) async fn promo_tier_upsert(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TierReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    if !(1..=8760).contains(&body.hours) {
        return Err(DomainError::Validation("时长需在 1-8760 小时".into()));
    }
    if body.price <= 0 {
        return Err(DomainError::Validation("价格需大于 0".into()));
    }
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM promo_kinds WHERE kind = $1)",
    )
    .bind(body.kind.trim())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !exists {
        return Err(DomainError::Validation("档位不存在".into()));
    }
    sqlx::query(
        "INSERT INTO promo_kind_tiers (kind, hours, price, enabled) \
         VALUES ($1, $2, $3, COALESCE($4, TRUE)) \
         ON CONFLICT (kind, hours) DO UPDATE SET price = EXCLUDED.price, \
           enabled = COALESCE($4, promo_kind_tiers.enabled)",
    )
    .bind(body.kind.trim())
    .bind(body.hours)
    .bind(body.price)
    .bind(body.enabled)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "promo_tier.upsert", None)
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/promo-kinds/tiers/{id}")]
pub(super) async fn promo_tier_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    let n = sqlx::query("DELETE FROM promo_kind_tiers WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state
        .repo
        .audit(Some(auth.id), "promo_tier.delete", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}
