//! 作品追更订阅（0450）：/me/work-subs 三件套。
//!
//! 粒度是作品键（WK），与作品页聚合用的是同一条键；派发在
//! `admin_http/review_side_effects.rs`（种子过审时按 work_key 反查订阅者）。
//! 这里只做用户的增删查——不动 WK 的算法，避免第二份口径。

use actix_web::{delete, get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 作品键上限：WK 是若干列拼出来的字符串，正常几十字符；
/// 给 400 的额度足够容纳长标题，同时挡掉把大段文本塞进来当键的用法。
const MAX_KEY_LEN: usize = 400;

#[derive(Deserialize)]
pub(crate) struct SubBody {
    pub work_key: String,
}

#[derive(Deserialize)]
pub(crate) struct SubQuery {
    /// 给了就顺带回答「我订阅了这部作品没有」
    pub work_key: Option<String>,
}

fn clean_key(raw: &str) -> DomainResult<&str> {
    let k = raw.trim();
    if k.is_empty() || k.len() > MAX_KEY_LEN {
        return Err(DomainError::Validation("作品键不合法".into()));
    }
    Ok(k)
}

/// 我的追更列表（按订阅时间倒序）；带 work_key 时附 subscribed 标记。
#[get("/me/work-subs")]
pub(crate) async fn work_sub_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<SubQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<(String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT work_key, created_at FROM work_subscriptions \
         WHERE user_id = $1 ORDER BY created_at DESC LIMIT 500",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut subscribed = false;
    if let Some(k) = q.work_key.as_deref().map(str::trim) {
        subscribed = rows.iter().any(|(key, _)| key == k);
    }
    Ok(ok(serde_json::json!({
        "subscribed": subscribed,
        "total": rows.len(),
        "items": rows
            .into_iter()
            .map(|(work_key, created_at)| serde_json::json!({
                "work_key": work_key,
                "created_at": created_at,
            }))
            .collect::<Vec<_>>(),
    })))
}

/// 订阅（幂等：重复订阅不报错，也不会把 created_at 刷成新的）。
#[post("/me/work-subs")]
pub(crate) async fn work_sub_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SubBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let key = clean_key(&body.work_key)?;
    sqlx::query(
        "INSERT INTO work_subscriptions (user_id, work_key) VALUES ($1, $2) \
         ON CONFLICT (user_id, work_key) DO NOTHING",
    )
    .bind(auth.id)
    .bind(key)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "subscribed": true })))
}

/// 取消订阅；不存在也算成功（幂等）。
#[delete("/me/work-subs")]
pub(crate) async fn work_sub_remove(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<SubQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let raw = q
        .work_key
        .as_deref()
        .ok_or_else(|| DomainError::Validation("缺少 work_key".into()))?;
    let key = clean_key(raw)?;
    sqlx::query(
        "DELETE FROM work_subscriptions WHERE user_id = $1 AND work_key = $2",
    )
    .bind(auth.id)
    .bind(key)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "subscribed": false })))
}

/// 路由自注册：mod.rs 已到行数基线，本模块自己把三条挂上，
/// 那边只需 `mod work_subs;` + `.configure(work_subs::configure)` 两行。
pub(crate) fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(work_sub_list)
        .service(work_sub_add)
        .service(work_sub_remove);
}
