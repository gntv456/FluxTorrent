//! M22 任务中心：任务列表/总览/认领。
//! 从 ops_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[get("/tasks")]
pub(super) async fn task_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let uid = require_auth(&req, &state).await.ok().map(|a| a.id);
    let rows = sqlx::query_as::<_, TaskRow>(
        "SELECT t.id, t.name, t.metric, t.reward, t.penalty, t.claim_limit, \
            (SELECT count(*) FROM task_claims tc WHERE tc.task_id = t.id)::bigint AS claimed, \
            t.starts_at, t.ends_at, t.tier, t.subtitle, t.fee, t.duration_days, \
            COALESCE(t.claim_limit, t.quota_total) AS quota_total, t.sort, \
            EXISTS(SELECT 1 FROM task_claims tc WHERE tc.task_id = t.id AND tc.user_id = $1) AS claimed_by_me \
         FROM tasks t WHERE now() BETWEEN t.starts_at AND t.ends_at \
         ORDER BY t.sort NULLS LAST, t.id",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub(super) struct TaskRow {
    id: i64,
    name: String,
    metric: serde_json::Value,
    reward: i64,
    penalty: i64,
    claim_limit: Option<i32>,
    claimed: i64,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
    /// 五档任务卡（包子站 task.php 口径）
    #[serde(skip_serializing_if = "Option::is_none")]
    tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subtitle: Option<String>,
    #[sqlx(default)]
    fee: i64,
    #[sqlx(default)]
    duration_days: i32,
    /// 0094 废弃：名额统一走 claim_limit；仅为 task-board 兼容下发（值 = claim_limit）
    #[sqlx(default)]
    quota_total: i32,
    #[sqlx(default)]
    sort: i32,
    /// 当前用户是否已领取
    #[sqlx(default)]
    claimed_by_me: bool,
}
