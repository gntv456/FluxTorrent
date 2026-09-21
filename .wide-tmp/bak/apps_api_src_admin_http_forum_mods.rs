use actix_web::{delete, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

#[derive(Deserialize)]
struct ForumUpsertReq {
    name: String,
    #[serde(default)]
    descr: Option<String>,
    #[serde(default)]
    minclassread: Option<i32>,
    #[serde(default)]
    minclasswrite: Option<i32>,
    #[serde(default)]
    minclasscreate: Option<i32>,
    #[serde(default)]
    protected: Option<bool>,
    /// 归属分区（0115）；None = 不分组
    #[serde(default)]
    category_id: Option<i64>,
}

fn forum_upsert_check(body: &ForumUpsertReq) -> DomainResult<()> {
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("版块名不能为空".into()));
    }
    let mr = body.minclassread.unwrap_or(0);
    let mw = body.minclasswrite.unwrap_or(0);
    let mc = body.minclasscreate.unwrap_or(0);
    if !(mr <= mw && mw <= mc) {
        return Err(DomainError::Validation(
            "三档门槛需满足 读 ≤ 回 ≤ 发".into(),
        ));
    }
    Ok(())
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
    forum_upsert_check(&body)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO forums (name, descr, minclassread, minclasswrite, minclasscreate, min_class, protected, category_id) \
         VALUES ($1, $2, $3, $4, $5, $3, $6, $7) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(&body.descr)
    .bind(body.minclassread.unwrap_or(0))
    .bind(body.minclasswrite.unwrap_or(0))
    .bind(body.minclasscreate.unwrap_or(0))
    .bind(body.protected.unwrap_or(false))
    .bind(body.category_id)
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
    forum_upsert_check(&body)?;
    let fid = path.into_inner();
    let n = sqlx::query(
        "UPDATE forums SET name = $1, descr = $2, \
            minclassread = $3, minclasswrite = $4, minclasscreate = $5, \
            min_class = $3, protected = $6, category_id = $7 \
         WHERE id = $8",
    )
    .bind(body.name.trim())
    .bind(&body.descr)
    .bind(body.minclassread.unwrap_or(0))
    .bind(body.minclasswrite.unwrap_or(0))
    .bind(body.minclasscreate.unwrap_or(0))
    .bind(body.protected.unwrap_or(false))
    .bind(body.category_id)
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
    sqlx::query("INSERT INTO forum_mods (forum_id, user_id, created_by) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
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
