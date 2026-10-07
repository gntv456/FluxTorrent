//! 作弊探测端点（0069 闭环 + 2026-10-07 保种组审计处置闭环）。
//! 从 misc_handlers.rs 拆出（300 行门禁）。

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;
use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::auth_infra::{require_auth, require_staff};

// ============ 作弊探测（0069 闭环查询端：tracker 拒绝 → worker 落库 → 后台可查） ============

#[derive(Deserialize)]
struct CheatEventsQuery {
    limit: Option<i64>,
    /// status=done 列已处置；缺省只列待处置（2026-10-07 保种组审计：处置待办视图）
    #[serde(default)]
    status: Option<String>,
}

/// cheat_events 列表（staff 专用）：按最近命中倒序。
/// 缺省只返回未处置行（resolved_at IS NULL）——累进处置（cheat_enforce）以
/// 「未处置事件」拉黑做种收益，处置待办是管理组的第一视图；status=done 看历史。
#[get("/admin/cheat-events")]
pub async fn cheat_events_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<CheatEventsQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_staff(&auth)?;
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let resolved = q.status.as_deref() == Some("done");
    let rows: Vec<(
        i64,
        i64,
        String,
        Option<String>,
        String,
        i64,
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT c.id, c.user_id, c.agent, c.peer_ip, c.reason, \
                c.hits, c.first_seen, c.last_seen, c.resolved_at, \
                u.username \
             FROM cheat_events c \
             LEFT JOIN users u ON u.id = c.user_id \
             WHERE COALESCE(c.resolved_at IS NULL, TRUE) = $2 \
             ORDER BY c.last_seen DESC LIMIT $1",
    )
    .bind(limit)
    .bind(!resolved)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = rows
        .into_iter()
        .map(
            |(
                id,
                uid,
                agent,
                ip,
                reason,
                hits,
                first,
                last,
                resolved,
                uname,
            )| {
                serde_json::json!({
                    "id": id, "user_id": uid, "username": uname,
                    "agent": agent, "peer_ip": ip,
                    "reason": reason, "hits": hits,
                    "first_seen": first, "last_seen": last,
                    "resolved_at": resolved,
                })
            },
        )
        .collect();
    Ok(ok(serde_json::json!({ "items": items })))
}

#[derive(Deserialize)]
struct CheatResolveReq {
    id: i64,
    /// true=标记已处置（恢复该用户做种收益结算）；false=撤销处置重新拉黑
    resolved: bool,
}

/// cheat_events 处置（staff）：置/清 resolved_at。
/// 拉黑口径见 worker seeding_reward——存在未处置 ghost/speed/reset 事件的
/// 用户停发做种收益；这里置位即恢复，撤销即重新拉黑。留审计。
#[post("/admin/cheat-events/resolve")]
pub async fn cheat_events_resolve(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CheatResolveReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_staff(&auth)?;
    // 状态卫语句与赋值分开绑定（$2 在两处语义不同：一个布尔一个时间戳，
    // 共用占位符会 boolean = timestamptz 类型错配——实测 500）
    let n = sqlx::query(
        "UPDATE cheat_events SET resolved_at = $2 \
         WHERE id = $1 AND (resolved_at IS NULL) = $3",
    )
    .bind(body.id)
    .bind(if body.resolved {
        Some(chrono::Utc::now())
    } else {
        None
    })
    .bind(body.resolved)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("事件不存在或状态未变化".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "cheat_event.resolve", Some(body.id))
        .await;
    Ok(ok(
        serde_json::json!({ "id": body.id, "resolved": body.resolved }),
    ))
}
