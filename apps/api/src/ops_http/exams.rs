//! 考核考试：我的考试记录。
//! 从 ops_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 我的考核进度：task_claims 关联 kind IN ('onboard','periodic') 的任务，
/// current 按 task_settle 同款口径实时计算（tier 非空 = 累计口径按现值）。
/// P-527：下载量原先不在这一屏的现值里（只有 uploaded/seed_seconds/
/// uploads/subtitles 四源），配了 `download_delta` 的考核项因此永远只能
/// 「只见目标」——而结算引擎早就按 `downloaded - base_downloaded` 判达标
/// （P-344），会员看不到自己被这条判据推到哪了。补齐第五源，口径与
/// `task_records.rs`（P-506）逐字相同。
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
                c.base_uploaded, c.base_downloaded, \
                c.base_seed_seconds, c.base_uploads, \
                t.tier, \
                (SELECT count(*) FROM subtitles sub WHERE sub.user_id = \
                 c.user_id AND sub.deleted_at IS NULL \
                 AND sub.created_at >= c.claimed_at) AS sub_since \
         FROM task_claims c JOIN tasks t ON t.id = c.task_id \
         WHERE c.user_id = $1 AND t.kind IN ('onboard','periodic') \
         ORDER BY c.claimed_at DESC LIMIT 100",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 指标现值一次性取齐（与 task_settle / task_records 相同的五个数据源）
    let stats: Option<(i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT u.uploaded, u.downloaded, \
                COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s WHERE s.user_id = u.id), 0)::bigint, \
                (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1), \
                (SELECT count(*) FROM subtitles sub WHERE sub.user_id = u.id \
                 AND sub.deleted_at IS NULL) \
         FROM users u WHERE u.id = $1",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let out: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            let current = stats.map(
                |(uploaded, downloaded, seed_secs, uploads, subtitles)| {
                    // 累计口径（tier 任务）：基线视为 0，直接报现值；否则报增量
                    let (up, dl, seed, ups) = if r.tier.is_some() {
                        (uploaded, downloaded, seed_secs, uploads)
                    } else {
                        (
                            uploaded - r.base_uploaded,
                            downloaded - r.base_downloaded,
                            seed_secs - r.base_seed_seconds,
                            uploads - r.base_uploads,
                        )
                    };
                    let subs = if r.tier.is_some() {
                        subtitles
                    } else {
                        // P-343：非 tier 只报认领后新增，与结算同窗
                        r.sub_since
                    };
                    serde_json::json!({
                        "uploaded": up,
                        "downloaded": dl,
                        "seed_seconds": seed,
                        "uploads": ups,
                        "subtitles": subs,
                    })
                },
            );
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
    base_downloaded: i64,
    base_seed_seconds: i64,
    base_uploads: i64,
    /// 认领后新增的字幕数（非 tier 任务的展示口径）
    sub_since: i64,
    tier: Option<String>,
}
