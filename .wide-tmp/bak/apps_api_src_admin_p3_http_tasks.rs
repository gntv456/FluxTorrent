//! P3-14 任务定义 CRUD
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

// ============ P3-14 任务定义 CRUD ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct TaskRow {
    id: i64,
    name: String,
    metric: serde_json::Value,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
    target_class: i32,
    reward: i64,
    penalty: i64,
    claim_limit: Option<i32>,
    #[sqlx(default)]
    kind: String,
    #[sqlx(default)]
    auto_assign: bool,
    #[sqlx(default)]
    period: String,
    // 以下 6 列为 0093 考核引擎字段，此前仅能改库、后台不可配（本次补齐）
    /// 考核期限（天）：认领时间 + duration_days = 截止时间，由 /me/exams 实时计算
    #[sqlx(default)]
    duration_days: i32,
    /// 副标题（列表展示用，如「注册后自动派发：做种满 120 小时即转正」）
    #[sqlx(default)]
    subtitle: Option<String>,
    /// 非空 = 累计口径（基线视为 0，直接报现值；空则报增量）
    #[sqlx(default)]
    tier: Option<String>,
    #[sqlx(default)]
    fee: i64,
    #[sqlx(default)]
    quota_total: i32,
    #[sqlx(default)]
    sort: i32,
}

#[get("/admin/tasks")]
async fn tasks_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<TaskRow> = sqlx::query_as(
        "SELECT id, name, metric, starts_at, ends_at, target_class, reward, penalty, claim_limit, \
                kind, auto_assign, period, \
                duration_days, subtitle, tier, fee, quota_total, sort \
         FROM tasks ORDER BY sort, id DESC",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
struct TaskReq {
    name: String,
    #[serde(default)]
    metric: Option<serde_json::Value>,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    target_class: Option<i32>,
    #[serde(default)]
    reward: Option<i64>,
    #[serde(default)]
    penalty: Option<i64>,
    #[serde(default)]
    claim_limit: Option<i32>,
    // 考核引擎（0093）：kind=task|onboard|periodic；auto_assign 自动派发；period=once|monthly|quarterly
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    auto_assign: Option<bool>,
    #[serde(default)]
    period: Option<String>,
    // 考核内容 / 方式的可配字段（本次补齐，此前这些列只能改库）
    /// 期限（天）：缺省沿用库内默认 30，不给则 UPSERT 用 COALESCE 保留原值
    #[serde(default)]
    duration_days: Option<i32>,
    /// 副标题：可传空串以清空（前端「清空」用）
    #[serde(default)]
    subtitle: Option<String>,
    /// 非空 = 累计口径；传空串清空
    #[serde(default)]
    tier: Option<String>,
    #[serde(default)]
    fee: Option<i64>,
    #[serde(default)]
    quota_total: Option<i32>,
    #[serde(default)]
    sort: Option<i32>,
}

#[post("/admin/tasks")]
async fn task_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TaskReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TASK_MANAGE)
        .await?;
    if body.name.trim().is_empty() || body.ends_at <= body.starts_at {
        return Err(DomainError::Validation(
            "任务名必填且结束时间需晚于开始".into(),
        ));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO tasks (name, metric, starts_at, ends_at, target_class, reward, penalty, claim_limit, \
                            kind, auto_assign, period, \
                            duration_days, subtitle, tier, fee, quota_total, sort) \
         VALUES ($1, COALESCE($2, '{}'::jsonb), $3, $4, COALESCE($5, 0), COALESCE($6, 0), COALESCE($7, 0), $8, \
                 COALESCE($9, 'task'), COALESCE($10, FALSE), COALESCE($11, 'once'), \
                 COALESCE($12, 30), $13, $14, COALESCE($15, 0), COALESCE($16, 200), COALESCE($17, 0)) \
         RETURNING id",
    )
    .bind(body.name.trim())
    .bind(body.metric.clone())
    .bind(body.starts_at)
    .bind(body.ends_at)
    .bind(body.target_class)
    .bind(body.reward)
    .bind(body.penalty)
    .bind(body.claim_limit)
    .bind(body.kind.as_deref().map(str::trim).filter(|k| !k.is_empty()))
    .bind(body.auto_assign)
    .bind(body.period.as_deref().map(str::trim).filter(|p| !p.is_empty()))
    .bind(body.duration_days)
    .bind(body.subtitle.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(body.tier.as_deref().map(str::trim).filter(|t| !t.is_empty()))
    .bind(body.fee)
    .bind(body.quota_total)
    .bind(body.sort)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "task.add", Some(id)).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/tasks/{id}")]
async fn task_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<TaskReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TASK_MANAGE)
        .await?;
    let id = path.into_inner();
    if body.ends_at <= body.starts_at {
        return Err(DomainError::Validation("结束时间需晚于开始".into()));
    }
    // subtitle / tier 直赋（允许清空）：后台表单每次都提交完整对象，故传 None 即为清空意图
    let n = sqlx::query(
        "UPDATE tasks SET name = $2, metric = COALESCE($3, metric), starts_at = $4, ends_at = $5, \
           target_class = COALESCE($6, target_class), reward = COALESCE($7, reward), \
           penalty = COALESCE($8, penalty), claim_limit = $9, \
           kind = COALESCE($10, kind), auto_assign = COALESCE($11, auto_assign), \
           period = COALESCE($12, period), \
           duration_days = COALESCE($13, duration_days), subtitle = $14, tier = $15, \
           fee = COALESCE($16, fee), quota_total = COALESCE($17, quota_total), sort = COALESCE($18, sort) \
         WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(body.metric.clone())
    .bind(body.starts_at)
    .bind(body.ends_at)
    .bind(body.target_class)
    .bind(body.reward)
    .bind(body.penalty)
    .bind(body.claim_limit)
    .bind(
        body.kind
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty()),
    )
    .bind(body.auto_assign)
    .bind(
        body.period
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty()),
    )
    .bind(body.duration_days)
    .bind(body.subtitle.clone())
    .bind(body.tier.clone())
    .bind(body.fee)
    .bind(body.quota_total)
    .bind(body.sort)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state
        .repo
        .audit(Some(auth.id), "task.update", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/tasks/{id}")]
async fn task_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TASK_MANAGE)
        .await?;
    let id = path.into_inner();
    let n = sqlx::query("DELETE FROM tasks WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id));
    }
    state.repo.audit(Some(auth.id), "task.del", Some(id)).await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}
