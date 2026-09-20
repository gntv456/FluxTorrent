//! 公告管理 + 趣味盒 + 友情链接（news/fun/linksmanage 复刻）。
//! 从 http.rs 按域拆出。

use actix_web::{delete, post, put, web, HttpRequest, Responder};
use serde::Deserialize;

// auth 模块经 state.jwt 使用（0071 RS256 化后 http 层不再直接调用）

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::auth_infra::require_auth;

// ============ 公告管理（news.php 复刻：发布/编辑/删除，staff 专用） ============

#[derive(Deserialize)]
struct NewsBody {
    title: String,
    body: String,
    #[serde(default)]
    badge: String,
}

#[post("/admin/news")]
pub async fn news_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<NewsBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_any_perm(
        &state,
        &auth,
        &[
            crate::authz::perm::NEWS_MANAGE,
            crate::authz::perm::ANNOUNCE_PUBLISH,
        ],
    )
    .await?;
    if body.title.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("标题和正文不能为空".into()));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO announcements (title, body, badge, author_id, sort)          VALUES ($1, $2, $3, $4, (SELECT COALESCE(max(sort),0)+10 FROM announcements)) RETURNING id",
    )
    .bind(body.title.trim())
    .bind(&body.body)
    .bind(if body.badge.trim().is_empty() { "公告" } else { body.badge.trim() })
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "news_create", None).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/news/{id}")]
pub async fn news_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<NewsBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_any_perm(
        &state,
        &auth,
        &[
            crate::authz::perm::NEWS_MANAGE,
            crate::authz::perm::ANNOUNCE_PUBLISH,
        ],
    )
    .await?;
    let updated =
        sqlx::query("UPDATE announcements SET title = $2, body = $3, badge = $4 WHERE id = $1")
            .bind(*path)
            .bind(body.title.trim())
            .bind(&body.body)
            .bind(if body.badge.trim().is_empty() {
                "公告"
            } else {
                body.badge.trim()
            })
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::NotFound(*path));
    }
    state.repo.audit(Some(auth.id), "news_update", None).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/news/{id}")]
pub async fn news_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_any_perm(
        &state,
        &auth,
        &[
            crate::authz::perm::NEWS_MANAGE,
            crate::authz::perm::ANNOUNCE_PUBLISH,
        ],
    )
    .await?;
    let deleted = sqlx::query("DELETE FROM announcements WHERE id = $1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if deleted.rows_affected() == 0 {
        return Err(DomainError::NotFound(*path));
    }
    state.repo.audit(Some(auth.id), "news_delete", None).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}
