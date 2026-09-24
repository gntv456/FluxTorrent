//! P3-12 Section 多维体系：分类模式 CRUD + is_custom_kind 维度判定。
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::staff;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

// ============ P3-12 Section 多维体系 ============

#[derive(sqlx::FromRow, serde::Serialize)]
pub(super) struct SectionModeRow {
    id: i32,
    name: String,
    show_source: bool,
    show_medium: bool,
    show_codec: bool,
    show_audio_codec: bool,
    show_standard: bool,
    show_processing: bool,
    show_team: bool,
    categories: i64,
}

#[get("/admin/section-modes")]
async fn section_modes_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<SectionModeRow> = sqlx::query_as(
        r#"SELECT m.id, m.name, m.show_source, m.show_medium, m.show_codec, m.show_audio_codec,
                  m.show_standard, m.show_processing, m.show_team,
                  (SELECT count(*) FROM categories c WHERE c.mode_id = m.id)::bigint AS categories
           FROM category_modes m ORDER BY m.id"#,
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
struct SectionModeReq {
    name: String,
    #[serde(default)]
    show_source: Option<bool>,
    #[serde(default)]
    show_medium: Option<bool>,
    #[serde(default)]
    show_codec: Option<bool>,
    #[serde(default)]
    show_audio_codec: Option<bool>,
    #[serde(default)]
    show_standard: Option<bool>,
    #[serde(default)]
    show_processing: Option<bool>,
    #[serde(default)]
    show_team: Option<bool>,
}

#[post("/admin/section-modes")]
async fn section_mode_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SectionModeReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("模式名不能为空".into()));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO category_modes (name, show_source, show_medium, show_codec, show_audio_codec, show_standard, show_processing, show_team) \
         VALUES ($1, COALESCE($2, TRUE), COALESCE($3, TRUE), COALESCE($4, TRUE), COALESCE($5, TRUE), COALESCE($6, TRUE), COALESCE($7, TRUE), COALESCE($8, TRUE)) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(body.show_source)
    .bind(body.show_medium)
    .bind(body.show_codec)
    .bind(body.show_audio_codec)
    .bind(body.show_standard)
    .bind(body.show_processing)
    .bind(body.show_team)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "section_mode.add", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/section-modes/{id}")]
async fn section_mode_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<SectionModeReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE category_modes SET name = $2, show_source = COALESCE($3, show_source), \
           show_medium = COALESCE($4, show_medium), show_codec = COALESCE($5, show_codec), \
           show_audio_codec = COALESCE($6, show_audio_codec), show_standard = COALESCE($7, show_standard), \
           show_processing = COALESCE($8, show_processing), show_team = COALESCE($9, show_team) WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(body.show_source)
    .bind(body.show_medium)
    .bind(body.show_codec)
    .bind(body.show_audio_codec)
    .bind(body.show_standard)
    .bind(body.show_processing)
    .bind(body.show_team)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "section_mode.update", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/section-modes/{id}")]
async fn section_mode_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    if id == 1 {
        return Err(DomainError::Validation("默认模式不可删除".into()));
    }
    let used: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM categories WHERE mode_id = $1",
    )
    .bind(id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    if used > 0 {
        sqlx::query("UPDATE categories SET mode_id = 1 WHERE mode_id = $1")
            .bind(id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    let n = sqlx::query("DELETE FROM category_modes WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "section_mode.del", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

/// 维度存在性判定（0085/0087）：kind 在 section_kinds 中即可用。
/// 0087 起 media/grades/editions 字典行已迁入 section_dict，九维全走统一通道，
/// legacy 实体表仅作历史口径存档（介质列保留兼容老数据）。
pub(crate) async fn is_custom_kind(db: &sqlx::PgPool, kind: &str) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM section_kinds WHERE kind = $1)",
    )
    .bind(kind)
    .fetch_one(db)
    .await
    .unwrap_or(false)
}
