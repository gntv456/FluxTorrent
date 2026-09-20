//! POST /admin/settings/validate：单字段预校验。
//! 从 settings_http.rs 按域拆出。

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::engine::validate_field;
use super::meta::{MetaRow, META_SELECT};

// POST /admin/settings/validate —— 单字段预校验
// ============================================================================

#[derive(Deserialize)]
struct ValidateReq {
    name: String,
    #[serde(default)]
    value: String,
}

#[post("/admin/settings/validate")]
pub(super) async fn settings_validate(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ValidateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    let name = body.name.trim();
    let meta: Option<MetaRow> =
        sqlx::query_as(&format!("{META_SELECT} WHERE s.name = $1"))
            .bind(name)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let meta = meta
        .ok_or_else(|| DomainError::Validation(format!("未知设定项 {name}")))?;
    match validate_field(&meta, &body.value) {
        Ok(normalized) => Ok(ok(serde_json::json!({
            "valid": true, "name": name, "normalized": normalized,
        }))),
        Err(e) => Err(DomainError::FieldErrors(vec![(name.to_string(), e)])),
    }
}

// ============================================================================
