//! 考核考试：我的考试记录。
//! 从 ops_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 我的考核进度：task_claims 关联 kind IN ('onboard','periodic') 的任务，
/// current 按 task_settle 同款口径实时计算（tier 非空 = 累计口径按现值）。
#[get("/me/exams")]
pub(super) async fn my_exams(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<MyExamRow> = sqlx::query_as(
        "SELECT t.id AS task_id, t.name, t.subtitle, t.metric, t.kind, t.period, \
                c.status, c.claimed_at, c.settled_at, \
                (c.claimed_at + (t.duration_days || ' days')::interval) AS deadline, \
                t.reward, t.penalty, \
                c.base_uploaded, c.base_seed_seconds, c.base_uploads, t.tier \
         FROM task_claims c JOIN tasks t ON t.id = c.task_id \
         WHERE c.user_id = $1 AND t.kind IN ('onboard','periodic') \
         ORDER BY c.claimed_at DESC LIMIT 100",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 指标现值一次性取齐（与 task_settle 相同的四个数据源）
    let stats: Option<(i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT u.uploaded, \
                COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s WHERE s.user_id = u.id), 0)::bigint, \
                (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1), \
                (SELECT count(*) FROM subtitles sub WHERE sub.user_id = u.id) \
         FROM users u WHERE u.id = $1",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let out: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            let current =
                stats.map(|(uploaded, seed_secs, uploads, subtitles)| {
                    // 累计口径（tier 任务）：基线视为 0，直接报现值；否则报增量
                    let (up, seed, ups) = if r.tier.is_some() {
                        (uploaded, seed_secs, uploads)
                    } else {
                        (
                            uploaded - r.base_uploaded,
                            seed_secs - r.base_seed_seconds,
                            uploads - r.base_uploads,
                        )
                    };
                    serde_json::json!({
                        "uploaded": up,
                        "seed_seconds": seed,
                        "uploads": ups,
                        "subtitles": subtitles,
                    })
                });
            serde_json::json!({
                "task_id": r.task_id,
                "name": r.name,
                "subtitle": r.subtitle,
                "metric": r.metric,
                "kind": r.kind,
                "period": r.period,
                "status": r.status,
                "claimed_at": r.claimed_at,
                "settled_at": r.settled_at,
                "deadline": r.deadline,
                "reward": r.reward,
                "penalty": r.penalty,
                "current": current.unwrap_or(serde_json::json!({})),
            })
        })
        .collect();
    Ok(ok(out))
}

// ============ M19 保种区 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct MyExamRow {
    task_id: i64,
    name: String,
    subtitle: Option<String>,
    metric: serde_json::Value,
    kind: String,
    period: String,
    status: i16,
    claimed_at: chrono::DateTime<chrono::Utc>,
    settled_at: Option<chrono::DateTime<chrono::Utc>>,
    deadline: Option<chrono::DateTime<chrono::Utc>>,
    reward: i64,
    penalty: i64,
    /// 基线快照（领取/派发时）：与现值相减得增量
    base_uploaded: i64,
    base_seed_seconds: i64,
    base_uploads: i64,
    tier: Option<String>,
}
