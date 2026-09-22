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
    /// 分区内排序（0154）
    #[sqlx(default)]
    sort: i32,
    topics: i64,
    /// 分区/节点（0115）
    #[sqlx(default)]
    category_id: Option<i64>,
    #[sqlx(default)]
    category_name: Option<String>,
}

/// 分区行（具名结构体）。
/// ⚠️ 曾用 `Vec<(i64, String, i32, bool)>` 元组，序列化成「数组的数组」，
/// 前端按 `c.id` / `c.name` 取值恒为 undefined → 分区名全空白。
/// 新增字段一律加在这里，前端与之一一对应，别再退回元组。
#[derive(serde::Serialize, sqlx::FromRow)]
struct ForumCategoryRow {
    id: i64,
    name: String,
    sort: i32,
    visible: bool,
    /// 该分区下的版块数（左栏徽标；不含在 forums 列表里的行也能显示计数）
    forums: i64,
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
        "SELECT f.id, f.name, f.descr, f.minclassread, f.minclasswrite, \
            f.minclasscreate, f.protected, f.sort, \
            (SELECT count(*) FROM topics t WHERE t.forum_id = f.id) AS topics, \
            f.category_id, c.name AS category_name \
         FROM forums f LEFT JOIN forum_categories c ON c.id = f.category_id \
         ORDER BY c.sort NULLS LAST, c.id, f.sort, f.id",
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
    let categories: Vec<ForumCategoryRow> = sqlx::query_as(
        "SELECT c.id, c.name, c.sort, c.visible, \
            (SELECT count(*) FROM forums f WHERE f.category_id = c.id) AS forums \
         FROM forum_categories c ORDER BY c.sort, c.id",
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
    let name = body.name.trim();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO forum_categories (name, sort, visible) VALUES \
         ($1, $2, $3) RETURNING id",
    )
    .bind(name)
    .bind(body.sort.unwrap_or(0))
    .bind(body.visible.unwrap_or(true))
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| super::forum_check::unique_or_internal(e, name))?;
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
    let name = body.name.trim();
    let n = sqlx::query(
        "UPDATE forum_categories SET name = $1, sort = $2, \
         visible = $3 WHERE id = $4",
    )
    .bind(name)
    .bind(body.sort.unwrap_or(0))
    .bind(body.visible.unwrap_or(true))
    .bind(cid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| super::forum_check::unique_or_internal(e, name))?
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

// ---- 分区批量排序（拖拽 / 上下移动后一次性落库） ----

#[derive(Deserialize)]
struct CategoryReorderReq {
    /// 目标顺序的分区 id 列表；下标即新的 sort 值
    ids: Vec<i64>,
}

/// 批量重排分区：`{"ids":[3,1,2]}` → id=3 得 sort=0、id=1 得 sort=1、id=2 得 sort=2。
/// 单条 UPDATE 完成，避免部分成功导致顺序错乱；未知 id 静默忽略。
#[post("/admin/forum-categories/reorder")]
async fn forum_category_reorder(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CategoryReorderReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    if body.ids.is_empty() {
        return Err(DomainError::Validation("分区顺序不能为空".into()));
    }
    let mut seen = std::collections::HashSet::new();
    if !body.ids.iter().all(|id| seen.insert(*id)) {
        return Err(DomainError::Validation("分区顺序中存在重复 id".into()));
    }
    let n = sqlx::query(
        "UPDATE forum_categories AS c SET sort = v.ord - 1 \
         FROM unnest($1::bigint[]) WITH ORDINALITY AS v(id, ord) \
         WHERE c.id = v.id",
    )
    .bind(&body.ids)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    state
        .repo
        .audit(Some(auth.id), "forum.category_reorder", None)
        .await;
    Ok(ok(serde_json::json!({ "updated": n })))
}
