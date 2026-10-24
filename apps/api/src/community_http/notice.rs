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
/// 0417 F10：回包从「仅存量覆盖」改为「全键位 + 用户覆盖」——此前用户一个
/// 开关没动过时回 {}，前端设置页无数据可渲染（空对象被当成「无此功能」）。
/// 键表与 notice_prefs_set 的白名单同源（16 键）。
#[utoipa::path(
    get,
    path = "/api/v1/me/notice-prefs",
    tag = "社区/通知",
    description = "我的通知偏好（16 档逐项开关；缺省=开）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/me/notice-prefs")]
pub(crate) async fn notice_prefs_get(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    const DEFAULT_ON: [&str; 16] = [
        "hr_prewarn",
        "hr_violation",
        "wishlist",
        "group_new_version",
        "network_new_release",
        "subgroup_new_release",
        "request_fulfilled",
        "resurrection",
        "class_promo",
        "gift",
        "comment_reply",
        "system",
        "donate_tier",
        "push_hr",
        "push_promo",
        "push_message",
    ];
    let prefs: serde_json::Value =
        sqlx::query_scalar("SELECT notice_prefs FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(serde_json::json!({}));
    let mut out = serde_json::Map::new();
    for k in DEFAULT_ON {
        let on = prefs
            .get(k)
            .and_then(|v| v.as_bool())
            .unwrap_or(true); // 缺省 = 开
        out.insert(k.to_string(), serde_json::json!(on));
    }
    Ok(ok(serde_json::Value::Object(out)))
}

#[derive(Deserialize, utoipa::ToSchema)]
struct NoticePrefsSetReq {
    key: String,
    enabled: bool,
}

/// 设置单项通知偏好（白名单键，防塞任意 JSON）
#[utoipa::path(
    post,
    path = "/api/v1/me/notice-prefs",
    tag = "社区/通知",
    description = "改通知偏好（逐项；必达类档位不可关）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[post("/me/notice-prefs")]
pub(crate) async fn notice_prefs_set(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<NoticePrefsSetReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    const KEYS: [&str; 16] = [
        "hr_prewarn",
        "hr_violation",
        "wishlist",
        "group_new_version",
        "network_new_release",
        "subgroup_new_release",
        "request_fulfilled",
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
#[utoipa::path(
    get,
    path = "/api/v1/pool/honor",
    tag = "社区/其它",
    description = "奖池荣誉榜（池子返还率受 PoolGuard 约束）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/pool/honor")]
pub(crate) async fn pool_honor(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    // P-521：这一口的 utoipa 契约自己声明了 `401 会话未登录（code 2001）`,
    // 代码里却一次登录都不验——匿名直接取到 50 名会员的
    // id/用户名/捐赠标记/本月与累计捐池火花。站点准入见 web/middleware.ts
    // 「未登录只放行登录页与静态资源」，荣誉榜也只挂在会员侧导航里。
    let _auth = require_auth(&req, &state).await?;
    // SUM() 出来是 NUMERIC：query_as 元组按 i64 解码会 22P02 类 500，显式 cast
    // P-529：荣誉榜原先直接查视图，把已停用/已墓碑的号当成"当前荣誉会员"
    // 展示并占排名位（现网实测 5 席里 4 席是被处置账号）。名录那条出口的
    // 规则 3（封号/封存/归档/墓碑不进列表）在这里同样成立 ⇒ 共用同一谓词。
    let rows: Vec<(i64, String, bool, i64, i64)> = sqlx::query_as(&format!(
        "SELECT h.id, h.username, h.donor, h.this_month::bigint, \
         h.total::bigint FROM v_pool_honor h \
         JOIN users u ON u.id = h.id \
         WHERE {} ORDER BY h.total DESC LIMIT 50",
        crate::repo::community::user_directory::member_visible("u")
    ))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}
