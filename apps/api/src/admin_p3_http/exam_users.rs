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
    /// 0481 收回标记：非空 = 已撤下（结算跳过、会员面板隐藏）
    canceled_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 考核记录浏览：kind IN ('onboard','periodic') 的 task_claims，可按用户/任务/状态筛选
#[get("/admin/exam-users")]
async fn exam_users(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TASK_MANAGE)
        .await?;
    let rows: Vec<ExamUserRow> = sqlx::query_as(
        "SELECT c.id AS claim_id, t.id AS task_id, t.name AS task_name, t.kind, t.period, \
                u.id AS user_id, u.username, c.status, c.claimed_at, c.settled_at, c.reward_paid, \
                c.exempted_at, c.canceled_at \
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

/// 收回（2026-11-08）：撤下错发/误派的考核记录。
/// open（status=0/2）：打 canceled 标记即同时离开结算流与会员面板；
/// 已结算（status=1）且发过奖：标记 + clawback_spark_tx 追回奖励
/// （地板 0、幂等键 exam_revoke:{claim_id}，流水与余额同一 applied 值）。
/// 记录行保留不 DELETE：exam_assign 的 NOT EXISTS 以行存在为界——删行
/// 会让窗口内用户在下个 tick 被重新派发，收回形同虚设。
#[post("/admin/exam-users/{claim_id}/revoke")]
async fn exam_user_revoke(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TASK_MANAGE)
        .await?;
    let claim_id = path.into_inner();
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let row: Option<(i64, i16, Option<i64>)> = sqlx::query_as(
        "SELECT user_id, status, reward_paid FROM task_claims \
         WHERE id = $1 FOR UPDATE",
    )
    .bind(claim_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((user_id, status, reward_paid)) = row else {
        return Err(DomainError::NotFound(claim_id));
    };
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM task_claims \
         WHERE id = $1 AND canceled_at IS NOT NULL)",
    )
    .bind(claim_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if already {
        return Err(DomainError::Validation("该考核记录已被收回".to_string()));
    }
    // 追回：只对真发出去过的奖励动手（reward_paid>0 即 settle_complete 落账）
    let mut clawback = 0i64;
    if status == 1 {
        if let Some(paid) = reward_paid.filter(|v| *v > 0) {
            clawback = crate::economy_http::clawback_spark_tx(
                &mut tx,
                user_id,
                paid,
                "exam_revoke",
                &format!("exam_revoke:{claim_id}"),
            )
            .await?;
        }
    }
    sqlx::query(
        "UPDATE task_claims SET canceled_at = now(), canceled_by = $2 \
         WHERE id = $1",
    )
    .bind(claim_id)
    .bind(auth.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "exam_user.revoke", Some(claim_id))
        .await;
    tracing::info!(claim_id, user_id, clawback, "exam claim revoked");
    Ok(ok(serde_json::json!({
        "claim_id": claim_id, "revoked": true, "clawback": clawback,
    })))
}
