//! 考核记录浏览（0093，对标 NP /user/exam-users）
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

// ============ 考核记录浏览（0093，对标 NP /user/exam-users） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct ExamUserRow {
    claim_id: i64,
    task_id: i64,
    task_name: String,
    kind: String,
    period: String,
    user_id: i64,
    username: String,
    status: i16,
    claimed_at: chrono::DateTime<chrono::Utc>,
    settled_at: Option<chrono::DateTime<chrono::Utc>>,
    reward_paid: Option<i64>,
    /// 0105 豁免标记：非空 = 暂不参与结算（对标 NP exam-users 的 avoid）
    exempted_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 考核记录浏览：kind IN ('onboard','periodic') 的 task_claims，可按用户/任务/状态筛选
#[get("/admin/exam-users")]
async fn exam_users(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TASK_MANAGE)
        .await?;
    let rows: Vec<ExamUserRow> = sqlx::query_as(
        "SELECT c.id AS claim_id, t.id AS task_id, t.name AS task_name, t.kind, t.period, \
                u.id AS user_id, u.username, c.status, c.claimed_at, c.settled_at, c.reward_paid, \
                c.exempted_at \
         FROM task_claims c \
         JOIN tasks t ON t.id = c.task_id \
         JOIN users u ON u.id = c.user_id \
         WHERE t.kind IN ('onboard','periodic') \
           AND ($1::bigint IS NULL OR u.id = $1) \
           AND ($2::bigint IS NULL OR t.id = $2) \
           AND ($3::smallint IS NULL OR c.status = $3) \
         ORDER BY c.claimed_at DESC LIMIT 200",
    )
    .bind(q.get("user_id").and_then(|v| v.parse::<i64>().ok()))
    .bind(q.get("task_id").and_then(|v| v.parse::<i64>().ok()))
    .bind(q.get("status").and_then(|v| v.parse::<i16>().ok()))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

/// 豁免（对标 NP exam-users 的 avoid）：暂不参与考核结算。
/// 不改 status 枚举（0105）—— task_settle 只扫 status=0，标记列方案下
/// 恢复后原记录自然回到结算流，无需状态迁移。
#[post("/admin/exam-users/{claim_id}/exempt")]
async fn exam_user_exempt(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TASK_MANAGE)
        .await?;
    let claim_id = path.into_inner();
    let n = sqlx::query(
        "UPDATE task_claims SET exempted_at = now(), exempted_by = $2 \
         WHERE id = $1 AND status = 0 AND exempted_at IS NULL",
    )
    .bind(claim_id)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(claim_id));
    }
    state
        .repo
        .audit(Some(auth.id), "exam_user.exempt", Some(claim_id))
        .await;
    Ok(ok(
        serde_json::json!({ "claim_id": claim_id, "exempted": true }),
    ))
}

/// 恢复（对标 NP exam-users 的 recover）：取消豁免，记录回到结算流。
#[post("/admin/exam-users/{claim_id}/recover")]
async fn exam_user_recover(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TASK_MANAGE)
        .await?;
    let claim_id = path.into_inner();
    let n = sqlx::query(
        "UPDATE task_claims SET exempted_at = NULL, exempted_by = NULL \
         WHERE id = $1 AND exempted_at IS NOT NULL",
    )
    .bind(claim_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(claim_id));
    }
    state
        .repo
        .audit(Some(auth.id), "exam_user.recover", Some(claim_id))
        .await;
    Ok(ok(
        serde_json::json!({ "claim_id": claim_id, "exempted": false }),
    ))
}
