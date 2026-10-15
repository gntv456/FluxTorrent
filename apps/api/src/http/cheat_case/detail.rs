//! GET /admin/cheat-cases/{id}：案件详情 = 案件 + 证据列表 + 备注时间线。
//! 从 cheat_case/mod.rs 拆出（300 行门禁）。

use actix_web::{get, web, HttpRequest, HttpResponse};

use super::super::auth_infra::{require_auth, require_staff};
use super::SLA_HOURS;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

#[get("/admin/cheat-cases/{id}")]
pub async fn cheat_case_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_staff(&auth)?;
    let id = path.into_inner();
    let case: Option<(
        i64,
        String,
        i16,
        Option<i32>,
        i16,
        Option<i64>,
        Option<String>,
        String,
        chrono::DateTime<chrono::Utc>,
        bool,
        Option<serde_json::Value>,
    )> = sqlx::query_as(
        "SELECT r.id, r.reason, r.priority, r.risk_score, r.status, \
                r.claimed_by, cu.username, u.username, r.created_at, \
                r.sla_breached, r.sanction \
           FROM reports r \
           JOIN users u ON u.id = r.ref_id \
           LEFT JOIN users cu ON cu.id = r.claimed_by \
           WHERE r.id = $1 AND r.ref_type = 'cheat'",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((
        cid,
        reason,
        priority,
        risk,
        st,
        claimed_by,
        claimed_name,
        uname,
        created,
        sla,
        sanction,
    )) = case
    else {
        return Err(DomainError::NotFound(id));
    };
    let evidence: Vec<(
        i64,
        String,
        Option<String>,
        String,
        i64,
        chrono::DateTime<chrono::Utc>,
        Option<chrono::DateTime<chrono::Utc>>,
    )> = sqlx::query_as(
        "SELECT c.id, c.agent, c.peer_ip, c.reason, c.hits, c.last_seen, \
                c.resolved_at \
           FROM cheat_events c JOIN reports r ON r.id = $1 \
           WHERE c.user_id = r.ref_id \
           ORDER BY c.resolved_at NULLS FIRST, c.last_seen DESC",
    )
    .bind(id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let notes: Vec<(i64, String, String, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT n.id, au.username, n.body, n.created_at \
               FROM case_notes n JOIN users au ON au.id = n.author_id \
              WHERE n.case_id = $1 ORDER BY n.created_at",
        )
        .bind(id)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "id": cid, "username": uname, "reason": reason,
        "priority": priority, "risk_score": risk,
        "status": st,
        "status_text": match st { 0 => "open", 2 => "claimed", _ => "done" },
        "claimed_by": claimed_by, "claimed_by_name": claimed_name,
        "created_at": created, "sla_breached": sla,
        "sla_hours": SLA_HOURS,
        "sanction": sanction,
        "evidence": evidence.iter().map(|(eid, agent, ip, r, hits, last, res)| {
            serde_json::json!({
                "id": eid, "agent": agent, "peer_ip": ip, "reason": r,
                "hits": hits, "last_seen": last, "resolved_at": res,
            })
        }).collect::<Vec<_>>(),
        "notes": notes.iter().map(|(nid, author, body, at)| {
            serde_json::json!({
                "id": nid, "author": author, "body": body, "created_at": at,
            })
        }).collect::<Vec<_>>(),
    })))
}
