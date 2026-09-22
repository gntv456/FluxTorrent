use actix_web::{delete, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::forum_check::{
    current_gates, ensure_category_exists, forum_upsert_check,
};
use super::guard::staff;

#[derive(Deserialize)]
pub(crate) struct ForumUpsertReq {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) descr: Option<String>,
    #[serde(default)]
    pub(crate) minclassread: Option<i32>,
    #[serde(default)]
    pub(crate) minclasswrite: Option<i32>,
    #[serde(default)]
    pub(crate) minclasscreate: Option<i32>,
    #[serde(default)]
    pub(crate) protected: Option<bool>,
    /// 归属分区（0115）；None = 不分组
    #[serde(default)]
    pub(crate) category_id: Option<i64>,
    /// 分区内排序（0154）
    #[serde(default)]
    pub(crate) sort: Option<i32>,
}

#[post("/admin/forums")]
async fn forum_admin_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ForumUpsertReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    let name = forum_upsert_check(&body)?;
    ensure_category_exists(&state.repo.db, body.category_id).await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO forums (name, descr, minclassread, minclasswrite, \
            minclasscreate, min_class, protected, category_id, sort) \
         VALUES ($1, $2, $3, $4, $5, $3, $6, $7, $8) RETURNING id",
    )
    .bind(&name)
    .bind(&body.descr)
    .bind(body.minclassread.unwrap_or(0))
    .bind(body.minclasswrite.unwrap_or(0))
    .bind(body.minclasscreate.unwrap_or(0))
    .bind(body.protected.unwrap_or(false))
    .bind(body.category_id)
    .bind(body.sort.unwrap_or(0))
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "forum.create", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/forums/{id}")]
async fn forum_admin_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<ForumUpsertReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    let name = forum_upsert_check(&body)?;
    let fid = path.into_inner();
    ensure_category_exists(&state.repo.db, body.category_id).await?;
    let (mr, mw, mc) = (
        body.minclassread.unwrap_or(0),
        body.minclasswrite.unwrap_or(0),
        body.minclasscreate.unwrap_or(0),
    );
    // 防「静默放宽权限」：编辑表单若不回填，会把三档门槛写成 0/0/0
    //（0 = 所有人可读/回/发）。这里留一条可追溯的审计。
    if let Some((br, bw, bc)) = current_gates(&state.repo.db, fid).await? {
        let was_open = (mr, mw, mc) == (0, 0, 0);
        if (br, bw, bc) != (0, 0, 0) && was_open {
            tracing::warn!(
                forum_id = fid,
                actor = auth.id,
                "版块三档门槛被清空为 0/0/0（权限放宽），请核对是否有意为之"
            );
            state
                .repo
                .audit(Some(auth.id), "forum.gates_cleared", Some(fid))
                .await;
        }
    }
    let n = sqlx::query(
        "UPDATE forums SET name = $1, descr = $2, \
            minclassread = $3, minclasswrite = $4, minclasscreate = $5, \
            min_class = $3, protected = $6, category_id = $7, sort = $8 \
         WHERE id = $9",
    )
    .bind(&name)
    .bind(&body.descr)
    .bind(mr)
    .bind(mw)
    .bind(mc)
    .bind(body.protected.unwrap_or(false))
    .bind(body.category_id)
    .bind(body.sort.unwrap_or(0))
    .bind(fid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(fid));
    }
    state
        .repo
        .audit(Some(auth.id), "forum.update", Some(fid))
        .await;
    Ok(ok(serde_json::json!({ "updated": fid })))
}

#[delete("/admin/forums/{id}")]
async fn forum_admin_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let force = q.get("force").map(|v| v == "true").unwrap_or(false);
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    let fid = path.into_inner();
    // 审计修复（P1）：删除版块会静默级联删除其下全部主题与帖子（topics del=c → posts 级联）。
    // 非空版块要求显式 force=true 才执行，防误删整版内容。
    let topic_cnt: i64 =
        sqlx::query_scalar("SELECT count(*) FROM topics WHERE forum_id = $1")
            .bind(fid)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    if topic_cnt > 0 && !force {
        return Err(DomainError::Validation(format!(
            "该版块仍有 {topic_cnt} 个主题（删除将级联清空全部帖子）。确认知悉请传 force=true"
        )));
    }
    let n = sqlx::query("DELETE FROM forums WHERE id = $1")
        .bind(fid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(fid));
    }
    state
        .repo
        .audit(Some(auth.id), "forum.delete", Some(fid))
        .await;
    Ok(ok(serde_json::json!({ "deleted": fid })))
}

/// 任命版主（无需等级）
#[derive(Deserialize)]
struct ForumModReq {
    username: String,
}

#[post("/admin/forums/{id}/mods")]
async fn forum_mod_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<ForumModReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    let fid = path.into_inner();
    let uid: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE username = $1 AND status < 2",
    )
    .bind(&body.username)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(uid) = uid else {
        return Err(DomainError::NotFound(0));
    };
    sqlx::query(
        "INSERT INTO forum_mods (forum_id, user_id, created_by) \
     VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(fid)
    .bind(uid)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "forum.mod_add", Some(uid))
        .await;
    Ok(ok(serde_json::json!({ "forum_id": fid, "user_id": uid })))
}

#[delete("/admin/forums/{id}/mods/{user_id}")]
async fn forum_mod_remove(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, i64)>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    let (fid, uid) = path.into_inner();
    sqlx::query("DELETE FROM forum_mods WHERE forum_id = $1 AND user_id = $2")
        .bind(fid)
        .bind(uid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "forum.mod_remove", Some(uid))
        .await;
    Ok(ok(serde_json::json!({ "removed": uid })))
}
