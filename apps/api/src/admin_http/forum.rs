use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

// ============ 论坛版块管理（forummanage.php 口径，forummanage ≥93） ============

/// 版块管理：三档门槛 + 受保护标记 + 版主名单
#[derive(serde::Serialize, sqlx::FromRow)]
struct ForumAdminRow {
    id: i64,
    name: String,
    descr: Option<String>,
    minclassread: i32,
    minclasswrite: i32,
    minclasscreate: i32,
    protected: bool,
    topics: i64,
    /// 分区/节点（0115）
    #[sqlx(default)]
    category_id: Option<i64>,
    #[sqlx(default)]
    category_name: Option<String>,
}

#[get("/admin/forums")]
async fn forum_admin_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    let rows: Vec<ForumAdminRow> = sqlx::query_as(
        "SELECT f.id, f.name, f.descr, f.minclassread, f.minclasswrite, f.minclasscreate, f.protected, \
            (SELECT count(*) FROM topics t WHERE t.forum_id = f.id) AS topics, \
            f.category_id, c.name AS category_name \
         FROM forums f LEFT JOIN forum_categories c ON c.id = f.category_id ORDER BY f.id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mods: Vec<(i64, i64, String)> = sqlx::query_as(
                "SELECT fm.forum_id, u.id, \
         u.username FROM forum_mods fm JOIN users u ON u.id = fm.user_id ORDER BY fm.forum_id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let categories: Vec<(i64, String, i32, bool)> = sqlx::query_as(
        "SELECT id, name, sort, \
         visible FROM forum_categories ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "forums": rows, "mods": mods, "categories": categories }),
    ))
}

// ---- 分区/节点管理（0115；与版块同权限档 FORUMS_MANAGE） ----

#[derive(Deserialize)]
struct CategoryUpsertReq {
    name: String,
    #[serde(default)]
    sort: Option<i32>,
    #[serde(default)]
    visible: Option<bool>,
}

#[post("/admin/forum-categories")]
async fn forum_category_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CategoryUpsertReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("分区名不能为空".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO forum_categories (name, sort, visible) VALUES \
         ($1, $2, $3) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(body.sort.unwrap_or(0))
    .bind(body.visible.unwrap_or(true))
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "forum.category_create", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/forum-categories/{id}")]
async fn forum_category_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<CategoryUpsertReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("分区名不能为空".into()));
    }
    let cid = path.into_inner();
    let n = sqlx::query(
        "UPDATE forum_categories SET name = $1, sort = $2, \
         visible = $3 WHERE id = $4",
    )
    .bind(body.name.trim())
    .bind(body.sort.unwrap_or(0))
    .bind(body.visible.unwrap_or(true))
    .bind(cid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(cid));
    }
    state
        .repo
        .audit(Some(auth.id), "forum.category_update", Some(cid))
        .await;
    Ok(ok(serde_json::json!({ "id": cid })))
}

#[delete("/admin/forum-categories/{id}")]
async fn forum_category_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    let cid = path.into_inner();
    // 删除分区不删版块：版块 category_id 走 ON DELETE SET NULL 回落「未分组」
    let n = sqlx::query("DELETE FROM forum_categories WHERE id = $1")
        .bind(cid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(cid));
    }
    state
        .repo
        .audit(Some(auth.id), "forum.category_delete", Some(cid))
        .await;
    Ok(ok(serde_json::json!({ "deleted": cid })))
}
