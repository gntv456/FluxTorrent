//! M16 咨询答复与处理（staff answer/mark/delete）。
//! 从 community_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
struct AnswerReq {
    id: i64,
    answer: String,
}

/// 答复（takeanswer 口径）：给来信人发一条私信 + 答复原文回写 staffmessages + 置已答复
#[post("/staffmessages/answer")]
async fn staff_answer(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AnswerReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::STAFF_MESSAGE,
    )
    .await?;
    if body.answer.trim().is_empty() {
        return Err(DomainError::Validation("答复内容不能为空".into()));
    }
    let orig: Option<(i64, String)> = sqlx::query_as(
        "SELECT user_id, \
         subject FROM staffmessages WHERE id = $1 AND answered = 0",
    )
    .bind(body.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((uid, subject)) = orig else {
        return Err(DomainError::Validation("来信不存在或已答复".into()));
    };
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // ① 给用户发私信（staff 直发，不走接收限制/防刷）
    sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body, location, saved, unread) \
         VALUES ($1, $2, $3, $4, 1, 1, true)",
    )
    .bind(auth.id)
    .bind(uid)
    .bind(format!("Re: {}", subject))
    .bind(body.answer.trim())
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // ② 回写答复原文 + 状态。
    // 审计修复（P1）：旧实现只写 answered=1 不动 ticket_status，"已答复"的单永远停在
    // "新"并被列表 ORDER BY ticket_status ASC 置顶。答复即推进到 2=已答复待确认。
    sqlx::query(
        "UPDATE staffmessages SET answered = 1, answered_by = $2, answer = $3, answered_at = now(), \
             ticket_status = CASE WHEN ticket_status < 2 THEN 2 ELSE ticket_status END \
         WHERE id = $1",
    )
    .bind(body.id)
    .bind(auth.id)
    .bind(body.answer.trim())
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "staffmsg_answer", Some(body.id))
        .await;
    Ok(ok(serde_json::json!({ "answered": true })))
}

#[derive(Deserialize)]
struct StaffMsgActionReq {
    ids: Vec<i64>,
}

/// 批量标记已答复
#[post("/staffmessages/mark")]
async fn staff_mark(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<StaffMsgActionReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::STAFF_MESSAGE,
    )
    .await?;
    // 审计修复（P1）：原单行 SQL 的续行反斜杠丢失，"END" 与 "WHERE" 粘连
    // （ENDWHERE）导致语法错 500。折行展开修复。
    let n = sqlx::query(
        "UPDATE staffmessages SET answered = 1, \
         answered_by = COALESCE(answered_by, $2), \
         answered_at = COALESCE(answered_at, now()), \
         ticket_status = CASE WHEN ticket_status < 2 THEN 2 \
         ELSE ticket_status END \
         WHERE id = ANY($1) AND answered = 0",
    )
    .bind(&body.ids)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    // 三轮遗留：批量标记/删除是管理动作但零审计（删除是物理 DELETE）
    if n > 0 {
        state.repo.audit(Some(auth.id), "staffmsg_mark", None).await;
    }
    Ok(ok(serde_json::json!({ "marked": n })))
}

/// 删除来信（单条/批量）
#[post("/staffmessages/delete")]
async fn staff_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<StaffMsgActionReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::STAFF_MESSAGE,
    )
    .await?;
    let n = sqlx::query("DELETE FROM staffmessages WHERE id = ANY($1)")
        .bind(&body.ids)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    // 三轮遗留：物理删除管理组工单来信必须留痕
    if n > 0 {
        state
            .repo
            .audit(Some(auth.id), "staffmsg_delete", None)
            .await;
    }
    Ok(ok(serde_json::json!({ "deleted": n })))
}
