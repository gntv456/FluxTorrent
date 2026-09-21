//! 工单体系。
//! 从 community_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ 工单体系（0078，U3D Ticket 口径，v3 §27-16） ============
// staffmessages 工单化：优先级/指派/四态流转，复用 STAFF_MESSAGE 权限与答复链路。

#[derive(sqlx::FromRow, serde::Serialize)]
struct TicketRow {
    id: i64,
    username: Option<String>,
    subject: String,
    priority: i16,
    assigned_to: Option<String>,
    ticket_status: i16,
    created_at: chrono::DateTime<chrono::Utc>,
    answered_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 工单列表（staff）：按状态过滤，优先级降序 + 新单在前
#[get("/stafftickets")]
async fn ticket_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::STAFF_MESSAGE,
    )
    .await?;
    let status: Option<i16> = q
        .get("status")
        .and_then(|s| s.parse::<i16>().ok())
        .filter(|s| (0..=3).contains(s));
    let rows = sqlx::query_as::<_, TicketRow>(
        "SELECT s.id, u.username, s.subject, s.priority, a.username AS assigned_to, \
                s.ticket_status, s.created_at, s.answered_at \
         FROM staffmessages s \
         LEFT JOIN users u ON u.id = s.user_id \
         LEFT JOIN users a ON a.id = s.assigned_to \
         WHERE ($1::smallint IS NULL OR s.ticket_status = $1) \
         ORDER BY s.ticket_status ASC, s.priority DESC, s.id DESC LIMIT 100",
    )
    .bind(status)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct TicketUpdateReq {
    id: i64,
    /// 0低 1中 2高 3紧急
    #[serde(default)]
    priority: Option<i16>,
    /// 0=新 1=处理中 2=已答复待确认 3=关闭
    #[serde(default)]
    ticket_status: Option<i16>,
    /// 指派给（用户名；空串=取消指派）
    #[serde(default)]
    assign: Option<String>,
}

/// 工单流转：改优先级/状态/指派（任意子集）。答复仍走既有 staff_answer（其顺带置 3）。
#[post("/stafftickets/update")]
async fn ticket_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TicketUpdateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::STAFF_MESSAGE,
    )
    .await?;
    if let Some(p) = body.priority {
        if !(0..=3).contains(&p) {
            return Err(DomainError::Validation("priority 需在 0-3".into()));
        }
    }
    if let Some(s) = body.ticket_status {
        if !(0..=3).contains(&s) {
            return Err(DomainError::Validation(
                "ticket_status 需在 0-3".into(),
            ));
        }
    }
    // 审计修复（P1）：工单状态机此前无任何流转约束——已答复/关闭的单可随意回 0，
    // "已答复"单永远停在"新"被置顶。现在：
    //   0新 → 1处理中 → 2已答复待确认 → 3关闭 单向推进；
    //   3关闭 仅允许显式重开回 1处理中（不允许回 0，保留处理轨迹）。
    if let Some(new_s) = body.ticket_status {
        let cur: Option<i16> = sqlx::query_scalar(
            "SELECT ticket_status FROM staffmessages WHERE id = $1",
        )
        .bind(body.id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .flatten();
        let Some(cur_s) = cur else {
            return Err(DomainError::Validation("工单不存在".into()));
        };
        let legal = new_s > cur_s || (cur_s == 3 && new_s == 1);
        if !legal {
            return Err(DomainError::Validation(format!(
                "非法状态流转：{cur_s} → {new_s}（工单状态只能单向推进；关闭单仅可重开为处理中）"
            )));
        }
    }
    let assignee: Option<i64> = match &body.assign {
        Some(name) if !name.trim().is_empty() => {
            let uid: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM users WHERE username = $1 AND class_id >= 50",
            )
            .bind(name.trim())
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
            let Some(uid) = uid else {
                return Err(DomainError::Validation(
                    "指派对象不存在或不是工作人员（class≥50）".into(),
                ));
            };
            Some(uid)
        }
        Some(_) => None, // 空串 = 清指派（置 NULL，见下方 clear_assign）
        None => {
            // 未传 assign = 不动：取当前值回写（COALESCE 不更新语义）
            let cur: Option<i64> = sqlx::query_scalar(
                "SELECT assigned_to FROM staffmessages WHERE id = $1",
            )
            .bind(body.id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
            cur
        }
    };
    // 审计修复（P1）：assign="" 走 COALESCE($4, assigned_to) 会被 NULL 吞回当前值，
    // 注释宣称的「空串=取消指派」从未生效。显式空串时改用 SET assigned_to = NULL。
    let clear_assign =
        matches!(body.assign.as_deref(), Some(s) if s.trim().is_empty());
    let n = if clear_assign {
        sqlx::query(
            "UPDATE staffmessages SET \
                priority = COALESCE($2, priority), \
                ticket_status = COALESCE($3, ticket_status), \
                assigned_to = NULL \
             WHERE id = $1",
        )
        .bind(body.id)
        .bind(body.priority)
        .bind(body.ticket_status)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected()
    } else {
        sqlx::query(
            "UPDATE staffmessages SET \
                priority = COALESCE($2, priority), \
                ticket_status = COALESCE($3, ticket_status), \
                assigned_to = COALESCE($4, assigned_to) \
             WHERE id = $1",
        )
        .bind(body.id)
        .bind(body.priority)
        .bind(body.ticket_status)
        .bind(assignee)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected()
    };
    if n == 0 {
        return Err(DomainError::NotFound(body.id));
    }
    state
        .repo
        .audit(Some(auth.id), "ticket_update", Some(body.id))
        .await;
    Ok(ok(serde_json::json!({ "id": body.id, "updated": n })))
}
