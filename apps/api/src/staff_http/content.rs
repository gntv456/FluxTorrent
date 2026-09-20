//! FAQ 管理（faqmanage）。
//! 从 staff_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct FaqRow {
    id: i32,
    category: String,
    question: String,
    answer: String,
    sort: i32,
}

#[get("/faq")]
pub async fn faq_list(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let rows: Vec<FaqRow> = sqlx::query_as(
        "SELECT id, category, question, answer, sort FROM faq_items ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct FaqBody {
    question: String,
    answer: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    sort: Option<i32>,
}

#[post("/admin/faq")]
pub async fn faq_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FaqBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FAQ_MANAGE)
        .await?;
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO faq_items (question, answer, category, sort) VALUES ($1, $2, $3, COALESCE($4, (SELECT max(sort)+1 FROM faq_items))) RETURNING id",
    )
    .bind(&body.question)
    .bind(&body.answer)
    .bind(if body.category.is_empty() { "default" } else { &body.category })
    .bind(body.sort)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "faq.create", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/faq/{id}")]
pub async fn faq_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<FaqBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FAQ_MANAGE)
        .await?;
    // sort 用 COALESCE 保留原值：编辑时不传 sort 不应把排序归零
    let n = sqlx::query("UPDATE faq_items SET question=$2, answer=$3, category=$4, sort=COALESCE($5, sort), updated_at=now() WHERE id=$1")
        .bind(*path).bind(&body.question).bind(&body.answer).bind(&body.category).bind(body.sort)
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if n.rows_affected() == 0 {
        return Err(DomainError::NotFound(*path as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "faq.update", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/faq/{id}")]
pub async fn faq_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FAQ_MANAGE)
        .await?;
    sqlx::query("DELETE FROM faq_items WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "faq.delete", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ---- 规则管理 ----
