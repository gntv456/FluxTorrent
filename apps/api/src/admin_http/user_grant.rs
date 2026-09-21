//! 用户详情页操作（勋章/道具/考核登记）。
//! 从 admin_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::{ensure_outranks, staff};

/// 详情页分配考核（参考站「分配考核」口径）：把用户登记为某考核岗位（jixiao_claims，当期）
#[derive(Deserialize)]
struct AssignJixiaoReq {
    type_id: i64,
    /// YYYY-MM；缺省当月
    #[serde(default)]
    period: Option<String>,
    #[serde(default)]
    amount: Option<i64>,
}

#[post("/admin/users/{id}/jixiao")]
async fn user_assign_jixiao(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<AssignJixiaoReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let uid = path.into_inner();
    // 等级护栏（审计修复）：90+ 可给 94/99 登记考核属越权，须严格高于目标
    ensure_outranks(&state.repo.db, auth.class_id, uid).await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM jixiao_types WHERE id = $1)",
    )
    .bind(body.type_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !exists {
        return Err(DomainError::NotFound(body.type_id));
    }
    let period = body.period.clone().unwrap_or_else(|| {
        (chrono::Utc::now() + chrono::Duration::hours(8))
            .format("%Y-%m")
            .to_string()
    });
    let dup: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM jixiao_claims WHERE user_id = $1 AND type_id = $2 AND period = $3)",
    )
    .bind(uid)
    .bind(body.type_id)
    .bind(&period)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if dup {
        return Err(DomainError::Validation(
            "该用户本期已登记此考核岗位".into(),
        ));
    }
    sqlx::query(
        // metrics_snapshot 记 source='admin'（0106 单行设计：登记与发放是同一行的
        // 状态流转，settled_at IS NULL 即未发放）。base_* 基线快照与批量分配
        // （/admin/jixiao/assign-batch）同口径——compute_metrics / jixiao_settle 的
        // 差值口径依赖它。
        "INSERT INTO jixiao_claims \
            (user_id, type_id, period, amount, metrics_snapshot, base_seed_seconds, base_uploaded, base_uploads) \
         SELECT $1, $2, $3, $4, '{\"source\":\"admin\"}'::jsonb, \
                COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s WHERE s.user_id = $1), 0)::bigint, \
                u.uploaded, \
                (SELECT count(*) FROM torrents tr WHERE tr.owner_id = $1 AND tr.approval_status = 1) \
         FROM users u WHERE u.id = $1",
    )
    .bind(uid)
    .bind(body.type_id)
    .bind(&period)
    .bind(body.amount.unwrap_or(0))
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "user.assign_jixiao", Some(uid))
        .await;
    Ok(ok(
        serde_json::json!({ "user_id": uid, "type_id": body.type_id, "period": period }),
    ))
}
