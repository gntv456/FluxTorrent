//! M16 短讯信箱读取（inbox/sent/staff）。
//! 从 community_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 三张信箱列表的单页上限（#70：此前三处硬 `LIMIT 100`，而角标是真计数）。
///
/// 现象：root 主收件箱 121 封（未读 120），角标与 SSE 都如实写 120，点进
/// 列表却只有 100 条——21 封未读在界面上根本不存在，标已读也标不到。
/// 真人用站就是这个体感：数字和列表对不上。
pub(crate) const MSG_PAGE_MAX: i64 = 100;

/// 缺省页大小。**不能改**：前端 `message-center.tsx` 把 `data` 当数组直接
/// 消费（`api.get<MessageRow[]>`），改成对象形状是破坏性变更；保持数组、
/// 只在显式给分页参数时翻页，现网前端零改动即不受影响。
pub(crate) const MSG_PAGE_DEFAULT: i64 = 100;

/// 响应形状：**仍是 `{code, data: [...]}` 数组**，另挂响应头
/// `X-Total-Count`（同筛选下的总条数）与 `X-Has-More`（本页是否还有下一批）。
///
/// 为什么是响应头而不是 `{items, total}`：消息中心、通知中心页、staff-box
/// 组件三处都直接把 `data` 当数组消费，改形状会同时打断三处，而且前端是
/// 站方在途件。头是这条链上唯一不破坏既有消费的形状。
fn page_envelope<T: serde::Serialize>(
    rows: Vec<T>,
    total: i64,
    limit: i64,
) -> actix_web::HttpResponse {
    let has_more = (rows.len() as i64) >= limit && total > (rows.len() as i64);
    // 走 ok() 而不是自己拼信封：信封还有 message / request_id 两字段
    // （dto::ok 从 request_id_mw 的 task-local 取本次 id），自拼会静默丢掉
    // 它们——前端与日志都靠这两个字段对账。
    let mut resp = ok(rows);
    let hdr = resp.headers_mut();
    hdr.insert(
        actix_web::http::header::HeaderName::from_static("x-total-count"),
        actix_web::http::header::HeaderValue::from_str(&total.to_string())
            .unwrap_or(actix_web::http::header::HeaderValue::from_static("0")),
    );
    hdr.insert(
        actix_web::http::header::HeaderName::from_static("x-has-more"),
        actix_web::http::header::HeaderValue::from_static(if has_more {
            "1"
        } else {
            "0"
        }),
    );
    resp
}

/// `limit` 的归一：缺省 100，越界夹到 [1, MSG_PAGE_MAX]。
fn page_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(MSG_PAGE_DEFAULT).clamp(1, MSG_PAGE_MAX)
}

/// 返回体：默认就是老形状的数组（前端无感）；给了 `limit`/`offset` 时才带
/// `X-Total-Count` 与 `X-Has-More` 两个头，让调用方能翻页而不用改数据形状。
///
/// 为什么要头而不是把 items/total 塞进 JSON：`/messages/inbox` 已被
/// notifications 页、staff-box 组件、message-center 直接 `.then(setRows)`
/// 当数组用，改形状会同时打断三处。头是这条链上唯一不破坏既有消费的形状。
#[derive(sqlx::FromRow, serde::Serialize)]
struct MessageRow {
    id: i64,
    counterpart: Option<String>,
    subject: String,
    body: String,
    read_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(default)]
    unread: Option<bool>,
    #[sqlx(default)]
    folder: Option<i32>,
    /// 系统通知（sender_id IS NULL）：前端据此做视觉区分（通知带 🔔 徽标、不可回复）
    #[sqlx(default)]
    is_system: bool,
    /// 文案键 + 参数（0288 / P1-9）：非空时前端按键现取字典渲染，切语言翻得动
    #[sqlx(default)]
    kind: Option<String>,
    #[sqlx(default)]
    params: Option<serde_json::Value>,
}

/// 收件箱（messages.php location=1 口径）：支持 box=folderid、关键词搜索（主题/正文/两者）与未读筛选
#[utoipa::path(
    get,
    path = "/api/v1/messages/inbox",
    tag = "社区/消息",
    description = "收件箱（分页；未读数随信封返回）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/messages/inbox")]
pub(crate) async fn message_inbox(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<InboxQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 关键词经 like_pattern 转义（%/_/\）后与原始串绑定：NULL 跳过、非 NULL 模糊匹配
    let pattern = q.search.as_deref().map(crate::http::like_pattern);
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT m.id, u.username AS counterpart, m.subject, m.body, m.read_at, m.created_at, m.unread, m.folder, \
                (m.sender_id IS NULL) AS is_system, m.kind, m.params \
         FROM messages m LEFT JOIN users u ON u.id = m.sender_id \
         WHERE m.receiver_id = $1 AND m.location = 1 \
           AND m.folder IS NOT DISTINCT FROM $2::int \
           AND ($3::text IS NULL OR m.subject ILIKE $3 OR m.body ILIKE $3) \
           AND ($4::bool IS NULL OR m.unread = $4) \
         ORDER BY m.id DESC LIMIT $5 OFFSET $6",
    )
    .bind(auth.id)
    .bind(q.box_id)
    .bind(pattern.as_deref())
    .bind(q.unread)
    .bind(page_limit(q.limit))
    .bind(q.offset.unwrap_or(0).max(0))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 角标是真计数（/me 的 unread_messages、SSE 的 unread 事件都是 count(*)），
    // 列表却在这里被截断：数字与列表对不上，用户点不完也标不完。取总数只为
    // 响应头，不进 body —— 前端三处仍按数组消费。
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM messages m \
         WHERE m.receiver_id = $1 AND m.location = 1 \
           AND m.folder IS NOT DISTINCT FROM $2::int \
           AND ($3::text IS NULL OR m.subject ILIKE $3 OR m.body ILIKE $3) \
           AND ($4::bool IS NULL OR m.unread = $4)",
    )
    .bind(auth.id)
    .bind(q.box_id)
    .bind(pattern.as_deref())
    .bind(q.unread)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(page_envelope(rows, total, page_limit(q.limit)))
}

/// 发件箱 / 管理组信箱的分页参数（#70：这两张表也是硬截断）。
#[derive(Deserialize, Default)]
#[serde(default)]
struct ListQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct InboxQuery {
    /// 自建文件夹 id（缺省 = 主收件箱 folder IS NULL）
    box_id: Option<i32>,
    search: Option<String>,
    unread: Option<bool>,
    /// 每页条数（#70）。缺省 100 = 老行为；给了就翻页，取回 X-Has-More。
    limit: Option<i64>,
    /// 偏移。与 limit 成对使用；深翻页请优先用 box/search/unread 缩小面。
    offset: Option<i64>,
}

/// 发件箱（saved=1 口径）
#[utoipa::path(
    get,
    path = "/api/v1/messages/sent",
    tag = "社区/消息",
    description = "发件箱。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/messages/sent")]
pub(crate) async fn message_sent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<ListQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let limit = page_limit(q.limit);
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT m.id, u.username AS counterpart, m.subject, m.body, m.read_at, m.created_at, m.unread, m.folder \
         FROM messages m LEFT JOIN users u ON u.id = m.receiver_id \
         WHERE m.sender_id = $1 AND m.saved = 1 ORDER BY m.id DESC LIMIT $2 OFFSET $3",
    )
    .bind(auth.id)
    .bind(limit)
    .bind(q.offset.unwrap_or(0).max(0))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM messages WHERE sender_id = $1 AND saved = 1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(page_envelope(rows, total, limit))
}

/// 管理组信箱（staffbox.php 口径）：与管理组成员（class_id >= 90）互发的短讯
#[utoipa::path(
    get,
    path = "/api/v1/messages/staff",
    tag = "社区/消息",
    description = "站内信-管理组信箱（发往管理组的消息）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/messages/staff")]
pub(crate) async fn message_staff(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<ListQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let limit = page_limit(q.limit).min(MSG_PAGE_DEFAULT);
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT m.id, u.username AS counterpart, m.subject, m.body, m.read_at, m.created_at \
         FROM messages m \
         LEFT JOIN users u ON u.id = CASE WHEN m.sender_id = $1 THEN m.receiver_id ELSE m.sender_id END \
         WHERE (m.sender_id = $1 AND EXISTS (SELECT 1 FROM users su WHERE su.id = m.receiver_id AND su.class_id >= 90)) \
            OR (m.receiver_id = $1 AND EXISTS (SELECT 1 FROM users su WHERE su.id = m.sender_id AND su.class_id >= 90)) \
         ORDER BY m.id DESC LIMIT $2 OFFSET $3",
    )
    .bind(auth.id)
    .bind(limit)
    .bind(q.offset.unwrap_or(0).max(0))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM messages m \
         WHERE (m.sender_id = $1 AND EXISTS (SELECT 1 FROM users su WHERE su.id = m.receiver_id AND su.class_id >= 90)) \
            OR (m.receiver_id = $1 AND EXISTS (SELECT 1 FROM users su WHERE su.id = m.sender_id AND su.class_id >= 90))",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(page_envelope(rows, total, limit))
}
