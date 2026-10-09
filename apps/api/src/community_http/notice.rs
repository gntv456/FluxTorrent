//! 通知偏好 + 站免池荣誉榜。
//! 从 community_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ 通知偏好 + 站免池荣誉榜（0075） ============

/// 我的通知偏好（缺省键 = 开）。已知事件类见 0075 迁移注释。
#[get("/me/notice-prefs")]
async fn notice_prefs_get(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let prefs: serde_json::Value =
        sqlx::query_scalar("SELECT notice_prefs FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(serde_json::json!({}));
    Ok(ok(prefs))
}

#[derive(Deserialize)]
struct NoticePrefsSetReq {
    key: String,
    enabled: bool,
}

/// 设置单项通知偏好（白名单键，防塞任意 JSON）
#[post("/me/notice-prefs")]
async fn notice_prefs_set(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<NoticePrefsSetReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    const KEYS: [&str; 14] = [
        "hr_prewarn",
        "hr_violation",
        "wishlist",
        "group_new_version",
        "network_new_release",
        "resurrection",
        "class_promo",
        "gift",
        "comment_reply",
        "system",
        "donate_tier",
        // Web Push 三 topic 的用户级开关（0283 P0-1；缺省 true）
        "push_hr",
        "push_promo",
        "push_message",
    ];
    if !KEYS.contains(&body.key.as_str()) {
        return Err(DomainError::Validation(format!(
            "未知通知类 {}（可选：{}）",
            body.key,
            KEYS.join("/")
        )));
    }
    sqlx::query(
        "UPDATE users SET notice_prefs = jsonb_set(notice_prefs, \
         ARRAY[$2], to_jsonb($3::boolean)) WHERE id = $1",
    )
    .bind(auth.id)
    .bind(&body.key)
    .bind(body.enabled)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "key": body.key, "enabled": body.enabled }),
    ))
}

/// 站免池贡献荣誉榜（0075，AB 池页口径：本月 + 累计，公开可见）
#[get("/pool/honor")]
async fn pool_honor(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    // SUM() 出来是 NUMERIC：query_as 元组按 i64 解码会 22P02 类 500，显式 cast
    let rows: Vec<(i64, String, bool, i64, i64)> = sqlx::query_as(
        "SELECT id, username, donor, this_month::bigint, \
         total::bigint FROM v_pool_honor ORDER BY total DESC LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}
