//! M16 短讯信箱读取（inbox/sent/staff）。
//! 从 community_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

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
#[get("/messages/inbox")]
async fn message_inbox(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<InboxQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 关键词经 like_pattern 转义（%/_/\）后与原始串绑定：NULL 跳过、非 NULL 模糊匹配
    let pattern = q.search.as_deref().map(crate::http::like_pattern);
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT m.id, u.username AS counterpart, m.subject, m.body, m.read_at, m.created_at, m.unread, m.folder, \
                (m.sender_id IS NULL) AS is_system, m.kind, m.params \
         FROM messages m LEFT JOIN users u ON u.id = m.sender_id \
         WHERE m.receiver_id = $1 AND m.location = 1 \
           AND ($2::int IS NULL OR m.folder = $2) \
           AND ($3::text IS NULL OR m.subject ILIKE $3 OR m.body ILIKE $3) \
           AND ($4::bool IS NULL OR m.unread = $4) \
         ORDER BY m.id DESC LIMIT 100",
    )
    .bind(auth.id)
    .bind(q.box_id)
    .bind(pattern.as_deref())
    .bind(q.unread)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct InboxQuery {
    /// 自建文件夹 id（缺省 = 主收件箱 folder IS NULL）
    box_id: Option<i32>,
    search: Option<String>,
    unread: Option<bool>,
}

/// 发件箱（saved=1 口径）
#[get("/messages/sent")]
async fn message_sent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT m.id, u.username AS counterpart, m.subject, m.body, m.read_at, m.created_at, m.unread, m.folder \
         FROM messages m LEFT JOIN users u ON u.id = m.receiver_id \
         WHERE m.sender_id = $1 AND m.saved = 1 ORDER BY m.id DESC LIMIT 100",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 管理组信箱（staffbox.php 口径）：与管理组成员（class_id >= 90）互发的短讯
#[get("/messages/staff")]
async fn message_staff(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT m.id, u.username AS counterpart, m.subject, m.body, m.read_at, m.created_at \
         FROM messages m \
         LEFT JOIN users u ON u.id = CASE WHEN m.sender_id = $1 THEN m.receiver_id ELSE m.sender_id END \
         WHERE (m.sender_id = $1 AND EXISTS (SELECT 1 FROM users su WHERE su.id = m.receiver_id AND su.class_id >= 90)) \
            OR (m.receiver_id = $1 AND EXISTS (SELECT 1 FROM users su WHERE su.id = m.sender_id AND su.class_id >= 90)) \
         ORDER BY m.id DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}
