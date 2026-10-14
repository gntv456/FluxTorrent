//! 后台运维·任务调度（0218 G7）：从 ops.rs 按域拆出（300 行门禁）。
//! 手动触发 = 入队 job_triggers，由 worker 在 60s tick 里用本体函数执行。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ ② 任务调度（0218 G7） ============
// 旧版 /admin/jobs/run + /run2 是「与 worker 同款 SQL 的复刻」（run2 自述如此）：
// job 演进后复刻口径会静默漂移，且只覆盖 10/40+ 个任务。现改为**入队**——
// POST 写 job_triggers，worker 在 60s tick 里用本体函数执行（同一把 advisory
// 锁，与定时触发互斥），结果写回该行；面板展示 = job_status（目录+最近运行）。
// 该跑什么、怎么跑，只有 worker 一个真相源。

#[derive(serde::Serialize, sqlx::FromRow)]
struct JobRow {
    job: String,
    cadence: String,
    last_started_at: Option<chrono::DateTime<chrono::Utc>>,
    last_finished_at: Option<chrono::DateTime<chrono::Utc>>,
    last_ok: Option<bool>,
    last_result: Option<String>,
    /// 0225 G30-B：最近一次执行的实例（多 worker 分实例可见）
    executed_by: String,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct TriggerRow {
    id: i64,
    job: String,
    status: String,
    requested_at: chrono::DateTime<chrono::Utc>,
    finished_at: Option<chrono::DateTime<chrono::Utc>>,
    ok: Option<bool>,
    result: Option<String>,
}

#[get("/admin/jobs")]
async fn admin_jobs_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CLEANUP_RUN)
        .await?;
    let jobs: Vec<JobRow> = sqlx::query_as(
        "SELECT job, cadence, last_started_at, last_finished_at, \
         last_ok, last_result, executed_by FROM job_status \
         ORDER BY cadence, job",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let triggers: Vec<TriggerRow> = sqlx::query_as(
        "SELECT id, job, status, requested_at, finished_at, ok, result \
         FROM job_triggers ORDER BY id DESC LIMIT 15",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "jobs": jobs, "triggers": triggers }),
    ))
}

#[derive(Deserialize)]
struct JobTriggerReq {
    job: String,
}

#[post("/admin/jobs/run")]
async fn admin_job_run(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<JobTriggerReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CLEANUP_RUN)
        .await?;
    let job = body.job.trim();
    // 目录校验：只有 worker 注册过的任务名可入队（防手填错名永远 pending）
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM job_status WHERE job = $1)",
    )
    .bind(job)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !exists {
        return Err(DomainError::Validation(
            "未知任务（worker 未注册该任务名）".to_string(),
        ));
    }
    // 连点合并：同任务已有待执行行则直接复用（多实例部署下也不重复排队）
    let pending: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM job_triggers WHERE job = $1 AND status = 'pending' \
         ORDER BY id LIMIT 1",
    )
    .bind(job)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(id) = pending {
        return Ok(ok(
            serde_json::json!({ "id": id, "job": job, "coalesced": true }),
        ));
    }
    // 队列上限：worker 每 tick 只认领 3 条，积压过多说明 worker 没在跑
    let queue: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM job_triggers \
         WHERE status IN ('pending', 'running')",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if queue >= 20 {
        return Err(DomainError::Validation(
            "触发队列已满（20 条待执行）——worker 可能未运行".to_string(),
        ));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO job_triggers (job, requested_by) VALUES ($1, $2) \
         RETURNING id",
    )
    .bind(job)
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // P-227：手动触发任务的审计要带任务名（哪个任务被人工插过一次队）
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "job_trigger",
            None,
            None,
            Some(serde_json::json!({"job": job})),
        )
        .await;
    Ok(ok(
        serde_json::json!({ "id": id, "job": job, "coalesced": false }),
    ))
}
