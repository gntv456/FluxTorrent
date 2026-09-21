//! P1-2 标签字典（参考站 tags 口径：名称 + 样式属性 + 作用域）+ 公开读
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

// ============ P1-2 标签字典（参考站 tags 口径：名称 + 样式属性 + 作用域） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct TagDictRow {
    id: i32,
    name: String,
    kind: String,
    /// 作用域（0138）：torrent=种子域 / forum=论坛域
    #[sqlx(default)]
    scope: String,
    bg_color: String,
    color: String,
    font_size: String,
    margin: String,
    padding: String,
    border_radius: String,
    sort: i32,
    enabled: bool,
    mode_id: Option<i32>,
}

#[derive(Deserialize)]
struct TagDictReq {
    name: String,
    #[serde(default)]
    kind: Option<String>,
    /// 作用域（0138）：缺省 torrent（存量口径），论坛标签选 forum
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    bg_color: Option<String>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    font_size: Option<String>,
    #[serde(default)]
    margin: Option<String>,
    #[serde(default)]
    padding: Option<String>,
    #[serde(default)]
    border_radius: Option<String>,
    #[serde(default)]
    sort: Option<i32>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    mode_id: Option<i32>,
}

#[get("/admin/tags-dict")]
async fn tags_dict_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let rows: Vec<TagDictRow> = sqlx::query_as(
        "SELECT id, name, kind, scope, bg_color, color, font_size, margin, padding, border_radius, sort, enabled, mode_id \
         FROM tag_dict ORDER BY scope, sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[post("/admin/tags-dict")]
async fn tags_dict_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TagDictReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let name = body.name.trim();
    if name.is_empty() {
        return Err(DomainError::Validation("标签名不能为空".into()));
    }
    // tag_dict.name 有唯一约束（0063）：预查重，撞键时给校验错误而非 500
    let dup: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM tag_dict WHERE name = $1)",
    )
    .bind(name)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if dup {
        return Err(DomainError::Validation("同名标签已存在".into()));
    }
    let next: i32 =
        sqlx::query_scalar("SELECT COALESCE(max(id), 0) + 1 FROM tag_dict")
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let scope = match body.scope.as_deref() {
        Some("forum") => "forum",
        _ => "torrent",
    };
    sqlx::query(
        "INSERT INTO tag_dict (id, name, kind, scope, bg_color, color, font_size, margin, padding, border_radius, sort, enabled, mode_id) \
         VALUES ($1, $2, COALESCE($3, 'plain'), $4, COALESCE($5, ''), COALESCE($6, '#ffffff'), \
                 COALESCE($7, '12px'), COALESCE($8, '0 4px 0 0'), COALESCE($9, '1px 4px'), \
                 COALESCE($10, '2px'), COALESCE($11, 0), COALESCE($12, TRUE), $13)",
    )
    .bind(next)
    .bind(name)
    .bind(body.kind.clone())
    .bind(scope)
    .bind(body.bg_color.clone())
    .bind(body.color.clone())
    .bind(body.font_size.clone())
    .bind(body.margin.clone())
    .bind(body.padding.clone())
    .bind(body.border_radius.clone())
    .bind(body.sort)
    .bind(body.enabled)
    .bind(body.mode_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "tagdict.add", Some(next as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": next })))
}

#[put("/admin/tags-dict/{id}")]
async fn tags_dict_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<TagDictReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    let scope = match body.scope.as_deref() {
        Some("forum") => "forum",
        _ => "torrent",
    };
    let n = sqlx::query(
        "UPDATE tag_dict SET name = $2, kind = COALESCE($3, kind), scope = $4, \
           bg_color = COALESCE($5, bg_color), color = COALESCE($6, color), font_size = COALESCE($7, font_size), \
           margin = COALESCE($8, margin), padding = COALESCE($9, padding), border_radius = COALESCE($10, border_radius), \
           sort = COALESCE($11, sort), enabled = COALESCE($12, enabled), mode_id = $13 \
         WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(body.kind.clone())
    .bind(scope)
    .bind(body.bg_color.clone())
    .bind(body.color.clone())
    .bind(body.font_size.clone())
    .bind(body.margin.clone())
    .bind(body.padding.clone())
    .bind(body.border_radius.clone())
    .bind(body.sort)
    .bind(body.enabled)
    .bind(body.mode_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "tagdict.update", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/tags-dict/{id}")]
async fn tags_dict_delete(
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
    sqlx::query("DELETE FROM tags WHERE tag_id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let n = sqlx::query("DELETE FROM tag_dict WHERE id = $1")
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
        .audit(Some(auth.id), "tagdict.del", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}
