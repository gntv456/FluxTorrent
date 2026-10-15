//! GET /admin/cheat-cases：案件队列（聚合未决证据数与 hits）。
//! 从 cheat_case/mod.rs 拆出（300 行门禁）。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::super::auth_infra::{require_auth, require_staff};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

#[derive(Deserialize)]
struct CheatCaseQuery {
    /// open=待处理（缺省）/ claimed=已认领 / done=已处理
    #[serde(default)]
    status: Option<String>,
    limit: Option<i64>,
}

#[get("/admin/cheat-cases")]
pub async fn cheat_case_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<CheatCaseQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_staff(&auth)?;
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let status: Vec<i16> = match q.status.as_deref() {
        Some("done") => vec![1],
        Some("claimed") => vec![2],
        Some("open") | None => vec![0],
        Some(other) => {
            return Err(DomainError::Validation(format!(
                "status 只认 open / claimed / done：{other}"
            )))
        }
    };
    let rows: Vec<(
        i64,
        i64,
        String,
        i16,
        Option<i32>,
        i16,
        Option<i64>,
        Option<String>,
        chrono::DateTime<chrono::Utc>,
        bool,
        i64,
        i64,
    )> = sqlx::query_as(
        "SELECT r.id, r.ref_id, u.username, r.priority, r.risk_score, \
                r.status, r.claimed_by, cu.username, r.created_at, \
                r.sla_breached, \
                (SELECT count(*) FROM cheat_events c \
                  WHERE c.user_id = r.ref_id AND c.resolved_at IS NULL), \
                (SELECT COALESCE(sum(c.hits), 0)::bigint FROM cheat_events c \
                  WHERE c.user_id = r.ref_id AND c.resolved_at IS NULL) \
           FROM reports r \
           JOIN users u ON u.id = r.ref_id \
           LEFT JOIN users cu ON cu.id = r.claimed_by \
           WHERE r.ref_type = 'cheat' AND r.status = ANY($1) \
           ORDER BY r.priority DESC, r.created_at LIMIT $2",
    )
    .bind(&status)
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = rows
        .into_iter()
        .map(
            |(
                id,
                uid,
                uname,
                priority,
                risk,
                st,
                claimed_by,
                claimed_name,
                created,
                sla,
                ev,
                hits,
            )| {
                serde_json::json!({
                    "id": id, "user_id": uid, "username": uname,
                    "priority": priority, "risk_score": risk,
                    "status": st,
                    "status_text": match st { 0 => "open", 2 => "claimed",
                        _ => "done" },
                    "claimed_by": claimed_by, "claimed_by_name": claimed_name,
                    "created_at": created, "sla_breached": sla,
                    "open_evidence": ev, "total_hits": hits,
                })
            },
        )
        .collect();
    Ok(ok(serde_json::json!({ "items": items })))
}
