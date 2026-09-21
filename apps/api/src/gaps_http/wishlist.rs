//! 心愿单（wishlist）端点。
//! 从 gaps_http/misc.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(sqlx::FromRow, serde::Serialize)]
struct WishRow {
    id: i64,
    keyword: String,
    category_id: Option<i32>,
    grade_id: Option<i32>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/wishlist")]
pub async fn wishlist_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<WishRow> = sqlx::query_as(
                "SELECT id, keyword, category_id, grade_id, \
         created_at FROM wishlist WHERE user_id = $1 ORDER BY id DESC LIMIT 100",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct WishAddReq {
    keyword: String,
    #[serde(default)]
    category_id: Option<i32>,
    #[serde(default)]
    grade_id: Option<i32>,
}

#[post("/wishlist")]
pub async fn wishlist_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WishAddReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let kw = body.keyword.trim();
    if kw.is_empty() || kw.len() > 100 {
        return Err(DomainError::Validation("关键词长度 1-100".into()));
    }
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM wishlist WHERE user_id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    if count >= 20 {
        return Err(DomainError::Validation(
            "愿望单上限 20 条，请先删除旧的".into(),
        ));
    }
    sqlx::query(
        "INSERT INTO wishlist (user_id, keyword, category_id, \
         grade_id) VALUES ($1, $2, $3, $4) ON CONFLICT (user_id, keyword) DO \
         UPDATE SET category_id = EXCLUDED.category_id, \
         grade_id = EXCLUDED.grade_id",
    )
    .bind(auth.id)
    .bind(kw)
    .bind(body.category_id)
    .bind(body.grade_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "keyword": kw })))
}

#[derive(Deserialize)]
struct WishDelReq {
    id: i64,
}

#[post("/wishlist/remove")]
pub async fn wishlist_remove(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WishDelReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let n = sqlx::query("DELETE FROM wishlist WHERE id = $1 AND user_id = $2")
        .bind(body.id)
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.id));
    }
    Ok(ok(serde_json::json!({ "removed": body.id })))
}
