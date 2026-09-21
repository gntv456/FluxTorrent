//! 趣味盒 + 友情链接（fun/linksmanage 复刻）。
//! 从 http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, Responder};
use serde::Deserialize;

// auth 模块经 state.jwt 使用（0071 RS256 化后 http 层不再直接调用）

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::auth_infra::require_auth;

// ============ 趣味盒（fun.php 复刻：浏览/投票/发布/编辑/删除/禁止） ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct FunItemRow {
    id: i32,
    username: Option<String>,
    title: String,
    body: Option<String>,
    status: String,
    added: chrono::DateTime<chrono::Utc>,
    #[sqlx(default)]
    fun_votes: Option<i64>,
    #[sqlx(default)]
    dull_votes: Option<i64>,
    #[sqlx(default)]
    my_vote: Option<String>,
}

/// 趣味盒列表（?status=all 全量需 staff；默认仅 normal）
#[get("/fun/items")]
pub async fn fun_items(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let staff_view = q.get("status").map(|s| s.as_str()).unwrap_or("") == "all";
    let rows: Vec<FunItemRow> = sqlx::query_as(
        "SELECT f.id, u.username, f.title, f.body, f.status, f.added, \
            (SELECT count(*) FROM fun_item_votes v WHERE v.fun_id = f.id AND v.vote = 'fun') AS fun_votes, \
            (SELECT count(*) FROM fun_item_votes v WHERE v.fun_id = f.id AND v.vote = 'dull') AS dull_votes, \
            (SELECT v.vote FROM fun_item_votes v WHERE v.fun_id = f.id AND v.user_id = $2) AS my_vote \
         FROM fun_items f LEFT JOIN users u ON u.id = f.user_id \
         WHERE ($3::bool OR f.status = 'normal') \
         ORDER BY f.added DESC LIMIT 50",
    )
    .bind(true)
    .bind(auth.id)
    .bind(auth.class_id >= 90 && staff_view)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct FunVoteBody {
    fun_id: i32,
    vote: String,
}

#[post("/fun/items/vote")]
pub async fn fun_item_vote(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FunVoteBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if body.vote != "fun" && body.vote != "dull" {
        return Err(DomainError::Validation("投票只能是 fun 或 dull".into()));
    }
    let inserted = sqlx::query(
        "INSERT INTO fun_item_votes (fun_id, user_id, vote) VALUES \
         ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(body.fun_id)
    .bind(auth.id)
    .bind(&body.vote)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if inserted.rows_affected() == 0 {
        return Err(DomainError::Validation("已经投过啦".into()));
    }
    Ok(ok(serde_json::json!({ "voted": body.vote })))
}

#[derive(Deserialize)]
struct FunItemBody {
    title: String,
    #[serde(default)]
    body: Option<String>,
}

/// 发布趣味内容（24h 冷却：最新一条发布不足 24 小时则拒绝，staff 豁免）
#[post("/fun/items")]
pub async fn fun_item_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FunItemBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if body.title.trim().is_empty() {
        return Err(DomainError::Validation("标题不能为空".into()));
    }
    let recent: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT max(added) FROM fun_items WHERE status NOT IN \
         ('banned','dull')",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(t) = recent {
        if chrono::Utc::now() - t < chrono::Duration::hours(24)
            && !crate::authz::can(&state, &auth, crate::authz::perm::FUN_MANAGE)
                .await
        {
            return Err(DomainError::Validation(
                "最新一条发布不足 24 小时，请稍后再来".into(),
            ));
        }
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO fun_items (user_id, title, body) VALUES ($1, $2, \
         $3) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.title.trim())
    .bind(&body.body)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/fun/items/{id}")]
pub async fn fun_item_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<FunItemBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let owner: Option<i64> =
        sqlx::query_scalar("SELECT user_id FROM fun_items WHERE id = $1")
            .bind(*path)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(*path as i64));
    };
    if owner != auth.id
        && !crate::authz::can(&state, &auth, crate::authz::perm::FUN_MANAGE)
            .await
    {
        return Err(DomainError::Forbidden);
    }
    sqlx::query("UPDATE fun_items SET title = $2, body = $3 WHERE id = $1")
        .bind(*path)
        .bind(body.title.trim())
        .bind(&body.body)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
struct FunStatusBody {
    status: String,
}

/// 修改状态（含「禁止」banned；staff 或作者对自己可 dull）
#[put("/fun/items/{id}/status")]
pub async fn fun_item_set_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<FunStatusBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let allowed =
        ["normal", "dull", "notfunny", "funny", "veryfunny", "banned"];
    if !allowed.contains(&body.status.as_str()) {
        return Err(DomainError::Validation("非法状态".into()));
    }
    let owner: Option<i64> =
        sqlx::query_scalar("SELECT user_id FROM fun_items WHERE id = $1")
            .bind(*path)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(*path as i64));
    };
    let is_staff = auth.class_id >= 90;
    if (body.status == "banned" || owner != auth.id) && !is_staff {
        return Err(DomainError::Forbidden);
    }
    sqlx::query("UPDATE fun_items SET status = $2 WHERE id = $1")
        .bind(*path)
        .bind(&body.status)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "fun_status_change", None)
        .await;
    Ok(ok(serde_json::json!({ "ok": true, "status": body.status })))
}

#[delete("/fun/items/{id}")]
pub async fn fun_item_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let owner: Option<i64> =
        sqlx::query_scalar("SELECT user_id FROM fun_items WHERE id = $1")
            .bind(*path)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(*path as i64));
    };
    if owner != auth.id
        && !crate::authz::can(&state, &auth, crate::authz::perm::FUN_MANAGE)
            .await
    {
        return Err(DomainError::Forbidden);
    }
    sqlx::query("DELETE FROM fun_items WHERE id = $1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "ok": true })))
}
