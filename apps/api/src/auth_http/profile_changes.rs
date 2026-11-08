//! 用户主页「变更记录」读口（0482c，自 profile.rs 拆出守行数门禁）：
//! audit_log 按目标用户回查，本人或职员（≥90）可见。本人视图隐去
//! `user.detail.view`（版主浏览足迹不对当事人展示，避免把审核动线反向
//! 暴露成「谁看过我」）；职员视图全量，口径同 /admin/audit?uid=。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(sqlx::FromRow, serde::Serialize)]
struct ChangeRow {
    id: i64,
    actor_id: Option<i64>,
    actor_username: Option<String>,
    action: String,
    detail: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/users/{id}/changes")]
pub async fn user_changes(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let uid = path.into_inner();
    let self_view = auth.id == uid;
    if !self_view && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    // 档案页同款防遍历：60 次/分 per-viewer（ref 回查带 LEFT JOIN，别更贵）
    crate::http::throttle_limited(&state, format!("changes:{}", auth.id), 60)
        .await?;
    let rows: Vec<ChangeRow> = sqlx::query_as(
        "SELECT a.id, a.actor_id, u.username AS actor_username, a.action, \
         a.ref::text AS detail, a.created_at FROM audit_log a \
         LEFT JOIN users u ON u.id = a.actor_id \
         WHERE a.ref->>'id' = $1 \
           AND ($2 = false OR a.action <> 'user.detail.view') \
         ORDER BY a.id DESC LIMIT 200",
    )
    .bind(uid.to_string())
    .bind(self_view)
    .fetch_all(&state.repo.read_db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}
