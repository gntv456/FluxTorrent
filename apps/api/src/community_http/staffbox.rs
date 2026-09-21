//! M16 咨询工作台（contactstaff → staffbox）。
//! 从 community_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ 咨询工作台（contactstaff → staffbox.php 口径） ============

#[derive(Deserialize)]
struct ContactReq {
    subject: String,
    body: String,
}

/// 用户提交咨询（takecontact.php 口径：写 staffmessages；普通用户 60s 防刷）
#[post("/contactstaff")]
async fn contact_staff(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ContactReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        let last: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
            "SELECT last_sent_at FROM message_flood WHERE user_id = $1",
        )
        .bind(auth.id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .flatten();
        if let Some(t) = last {
            if (chrono::Utc::now() - t).num_seconds() < 60 {
                return Err(DomainError::Validation(
                    "发送过于频繁，请 1 分钟后再试".into(),
                ));
            }
        }
    }
    if body.subject.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("主题与正文不能为空".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO staffmessages (user_id, subject, body) VALUES \
         ($1, $2, $3) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.subject.trim())
    .bind(body.body.trim())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO message_flood (user_id, last_sent_at) VALUES ($1, now()) \
                 ON CONFLICT (user_id) DO UPDATE SET last_sent_at = now()",
    )
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .ok();
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct StaffMessageRow {
    id: i64,
    username: Option<String>,
    subject: String,
    body: String,
    answered: i32,
    answered_by: Option<String>,
    answer: Option<String>,
    answered_at: Option<chrono::DateTime<chrono::Utc>>,
    permission: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// 咨询列表（staffbox.php）：全量给 staff（≥90）；permission 标签按标签名分流——
/// 与 NP 的权限位映射不同，这里简化为「全员可见普通咨询 + 带 permission 标签的定向分流
/// 也全员可见」，保留字段供未来按权限位过滤。
#[get("/staffmessages")]
async fn staff_messages(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<StaffMsgQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::STAFF_MESSAGE,
    )
    .await?;
    // answered 解析：0/1/true/false → Option<i32>（非法值按未过滤处理，不再裸 400）
    let answered_i: Option<i32> = q.answered.as_deref().and_then(|v| match v {
        "0" | "false" => Some(0),
        "1" | "true" => Some(1),
        _ => v.parse::<i32>().ok().filter(|n| (0..=1).contains(n)),
    });
    let rows = sqlx::query_as::<_, StaffMessageRow>(
        "SELECT s.id, u.username, s.subject, s.body, s.answered, a.username AS answered_by, \
                s.answer, s.answered_at, s.permission, s.created_at \
         FROM staffmessages s \
         LEFT JOIN users u ON u.id = s.user_id \
         LEFT JOIN users a ON a.id = s.answered_by \
         WHERE ($1::int IS NULL OR s.answered = $1) \
         ORDER BY s.answered ASC, s.id DESC LIMIT 100",
    )
    .bind(answered_i)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct StaffMsgQuery {
    /// 审计修复（P2 信封）：serde 对 ?answered=false 直接反序列化报裸文本 400（击穿
    /// JSON 信封）。改为 String 自行解析，支持 0/1/true/false 四种形态。
    answered: Option<String>,
}

/// 我的工单（用户侧，审计修复 P1）：普通用户此前看不到自己提交的咨询进度——
/// contactstaff 提交后只能等 PM，状态 2（已答复待确认）也无法自行确认关闭。
/// 仅返回本人提交的记录（staff 视角走 /stafftickets）。
#[derive(sqlx::FromRow, serde::Serialize)]
struct MyTicketRow {
    id: i64,
    subject: String,
    body: String,
    ticket_status: i16,
    priority: i16,
    answer: Option<String>,
    answered_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/me/staffmessages")]
async fn my_staff_messages(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<MyTicketRow> = sqlx::query_as(
        "SELECT id, subject, body, ticket_status, priority, answer, answered_at, created_at \
         FROM staffmessages WHERE user_id = $1 ORDER BY id DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 用户确认关闭工单（2 已答复待确认 → 3 关闭，闭环补全）：仅来信人本人可关。
#[derive(Deserialize)]
struct TicketConfirmReq {
    id: i64,
}

#[post("/me/staffmessages/confirm")]
async fn my_ticket_confirm(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TicketConfirmReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let n = sqlx::query(
        "UPDATE staffmessages SET ticket_status = 3 \
         WHERE id = $1 AND user_id = $2 AND ticket_status = 2",
    )
    .bind(body.id)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation(
            "工单不存在、非本人或尚未答复（仅已答复待确认的工单可确认关闭）"
                .into(),
        ));
    }
    Ok(ok(serde_json::json!({ "id": body.id, "ticket_status": 3 })))
}
