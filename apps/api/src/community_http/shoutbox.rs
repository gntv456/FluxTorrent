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

#[utoipa::path(
    get,
    path = "/api/v1/shoutbox",
    tag = "社区/喊话",
    description = "喊话列表（滚动播报）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/shoutbox")]
pub(crate) async fn shoutbox_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    // 审计修复（P1）：与全站鉴权口径对齐——聊天记录含用户名与发言内容，不应对匿名开放
    let _auth = require_auth(&req, &state).await?;
    let rows: Vec<ShoutRow> = sqlx::query_as(
                "SELECT sb.id, u.username, sb.message, \
         sb.created_at FROM shoutbox sb LEFT JOIN users u ON u.id = sb.user_id ORDER BY sb.id DESC LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize, utoipa::ToSchema)]
struct ShoutReq {
    message: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/shoutbox",
    tag = "社区/喊话",
    description = "发喊话（每 10 秒一条；被禁言者不可发；不收费）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[post("/shoutbox")]
pub(crate) async fn shoutbox_send(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ShoutReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.message.trim().is_empty() || body.message.chars().count() > 300 {
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
    // 防刷：每用户 10 秒冷却。
    // P-73：旧写法两处同族病——`incr(...).unwrap_or(0)` 在 Redis 抖动时
    // 计数当成 0（冷却形同不存在），而 `if n == 1 { expire }` 一失败就留下
    // 一个**永不过期**的计数键 ⇒ 该用户从那一刻起永久 429，刷不了屏也发不了
    // 言，只能等运维手删键。统一走 rate_window：fail-close + 窗口写进键名。
    crate::http::rate_window(&state, &format!("shout:{}", auth.id), 1, 10)
        .await?;
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
#[derive(Deserialize, utoipa::ToSchema)]
struct ShoutDeleteReq {
    id: i64,
}

#[utoipa::path(
    post,
    path = "/api/v1/shoutbox/delete",
    tag = "社区/喊话",
    description = "删喊话（本人 2 分钟内可撤回；职员不受时限）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[post("/shoutbox/delete")]
pub(crate) async fn shoutbox_delete(
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

/// 清空聊天室（staff 专属，0406 纯净化配套：纯聊天室需要一个整体的
/// 「翻页」口——逐条删 50 次不现实）。全量 DELETE + 审计；幂等
///（空表也 200，cleared=0）。
#[utoipa::path(
    post,
    path = "/api/v1/shoutbox/clear",
    tag = "社区/喊话",
    description = "清空聊天室全部发言（仅职员）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 403, description = "无权限（code 2003）")))]
#[post("/shoutbox/clear")]
pub(crate) async fn shoutbox_clear(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let n = sqlx::query("DELETE FROM shoutbox")
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    state
        .repo
        .audit(Some(auth.id), "shoutbox.clear", None)
        .await;
    Ok(ok(serde_json::json!({ "cleared": n })))
}
