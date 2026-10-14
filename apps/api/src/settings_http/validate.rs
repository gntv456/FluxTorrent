//! POST /admin/settings/validate：单字段预校验。
//! 从 settings_http.rs 按域拆出。

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use sqlx::PgPool;

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

/// 站点设定的唯一一把校验尺（批四十八 P-186）：预检端点与
/// `PUT /admin/settings` 必须走同一个函数。原先只有这里在判 number/整数/范围/
/// 必填/正则，写入口只判长度与 `site_type` —— 于是「面板预检说合法」与「实际
/// 写得进去」是两套规则，脏值进了库后读端 `.parse().unwrap_or(默认)` 静默回落，
/// 站长看到的是「改了没生效」。
///
/// 返回 `(归一化后的值, 是否 secret)`；secret 决定审计里要不要写出明文。
pub(crate) async fn validate_setting(
    db: &PgPool,
    name: &str,
    raw: &str,
) -> DomainResult<(String, bool)> {
    let meta: Option<MetaRow> =
        sqlx::query_as(&format!("{META_SELECT} WHERE s.name = $1"))
            .bind(name)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let meta = meta
        .ok_or_else(|| DomainError::Validation(format!("未知设定项 {name}")))?;
    validate_field(&meta, raw)
        .map(|v| (v, meta.secret))
        .map_err(|e| DomainError::FieldErrors(vec![(name.to_string(), e)]))
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
    let (normalized, _) =
        validate_setting(&state.repo.db, name, &body.value).await?;
    Ok(ok(serde_json::json!({
        "valid": true, "name": name, "normalized": normalized,
    })))
}

// ============================================================================
