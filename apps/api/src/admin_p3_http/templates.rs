//! P2-11 消息模板新增 / 删除
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{delete, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

// ============ P2-11 消息模板新增 / 删除 ============

#[derive(Deserialize)]
struct TemplateCreateReq {
    scene_key: String,
    subject: String,
    body: String,
    #[serde(default)]
    note: Option<String>,
}

#[post("/admin/message-templates")]
async fn admin_template_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TemplateCreateReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // 审计修复：消息模板是站点级配置，须 SETTINGS_MANAGE
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let scene = body.scene_key.trim();
    if scene.is_empty()
        || body.subject.trim().is_empty()
        || body.body.trim().is_empty()
    {
        return Err(DomainError::Validation("场景键/主题/正文必填".into()));
    }
    if !scene
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(DomainError::Validation(
            "场景键仅允许小写字母/数字/下划线".into(),
        ));
    }
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM message_templates WHERE scene_key = $1)",
    )
    .bind(scene)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if exists {
        return Err(DomainError::Validation("场景键已存在".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO message_templates (scene_key, subject, body, note) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(scene)
    .bind(body.subject.trim())
    .bind(body.body.trim())
    .bind(body.note.clone())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "template.create", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[delete("/admin/message-templates/{id}")]
async fn admin_template_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // 审计修复：消息模板是站点级配置，须 SETTINGS_MANAGE
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    let n = sqlx::query("DELETE FROM message_templates WHERE id = $1")
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
        .audit(Some(auth.id), "template.delete", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}
