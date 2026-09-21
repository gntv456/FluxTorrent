//! M16 聊天盒（shoutbox + 机器人）。
//! 从 community_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ M16 短讯与好友 ============

// （我的 H&R /me/hr 由 gaps_http::my_hr_status 提供——hr_snapshots 口径；
//  旧 snatches 口径实现已删除，曾与前者重复注册同一路由）

// ============ 聊天盒（shoutbox.php 口径：最近消息 + 发言） ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct ShoutRow {
    id: i64,
    username: Option<String>,
    message: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/shoutbox")]
async fn shoutbox_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    // 审计修复（P1）：与全站鉴权口径对齐——聊天记录含用户名与发言内容，不应对匿名开放
    let _auth = require_auth(&req, &state).await?;
    let rows: Vec<ShoutRow> = sqlx::query_as(
        "SELECT sb.id, u.username, sb.message, sb.created_at          FROM shoutbox sb LEFT JOIN users u ON u.id = sb.user_id          ORDER BY sb.id DESC LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct ShoutReq {
    message: String,
}

#[post("/shoutbox")]
async fn shoutbox_send(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ShoutReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.message.trim().is_empty() || body.message.len() > 300 {
        return Err(DomainError::Validation("发言需 1-300 字".into()));
    }
    // 禁言位（NP chatpost 口径）：被禁言用户不能在聊天室继续刷屏
    let can_chat: bool = sqlx::query_scalar(
        "SELECT COALESCE(forumpost, TRUE) FROM users WHERE id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !can_chat {
        return Err(DomainError::Validation("你已被禁言".into()));
    }
    // 防刷：每用户 10 秒冷却（Redis 计数，首条设 TTL；故障静默放行不影响可用性）
    {
        let mut c = state.redis.clone();
        let key = format!("rl:shout:{}", auth.id);
        let n: i64 = redis::AsyncCommands::incr(&mut c, &key, 1)
            .await
            .unwrap_or(0);
        if n == 1 {
            let _: () = redis::AsyncCommands::expire(&mut c, &key, 10)
                .await
                .unwrap_or(());
        }
        if n > 1 {
            return Err(DomainError::RateLimited);
        }
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO shoutbox (user_id, message) VALUES ($1, $2) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.message.trim())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

/// 删除聊天发言（staff：审计修复 P1——此前全后端无 DELETE FROM shoutbox，
/// 违规发言只能等禁言、无法清除已发内容）。发言本人 2 分钟内也可撤回。
#[derive(Deserialize)]
struct ShoutDeleteReq {
    id: i64,
}

#[post("/shoutbox/delete")]
async fn shoutbox_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ShoutDeleteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let row: Option<(i64, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT user_id, created_at FROM shoutbox WHERE id = $1",
    )
    .bind(body.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((uid, at)) = row else {
        return Err(DomainError::NotFound(body.id));
    };
    let is_staff = auth.class_id >= 90;
    let own_recent =
        uid == auth.id && (chrono::Utc::now() - at).num_seconds() <= 120;
    if !is_staff && !own_recent {
        return Err(DomainError::Forbidden);
    }
    sqlx::query("DELETE FROM shoutbox WHERE id = $1")
        .bind(body.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if is_staff && uid != auth.id {
        state
            .repo
            .audit(Some(auth.id), "shoutbox_delete", Some(body.id))
            .await;
    }
    Ok(ok(serde_json::json!({ "deleted": body.id })))
}
