//! 我的任务记录（拆自 task_overview.rs，M22）：认领行的实时进度。
//!
//! 尺子必须与 `apps/worker/src/task_jobs/settle.rs` 逐键一致——面板报
//! 「3/3 达标」而结算判 0/3，任务会挂到到期还倒扣罚金（P-343 字幕、
//! P-344 download_delta 都是这样暴露出来的）。

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

#[derive(sqlx::FromRow)]
struct MineRow {
    id: i64,
    name: String,
    status: i16,
    claimed_at: chrono::DateTime<chrono::Utc>,
    settled_at: Option<chrono::DateTime<chrono::Utc>>,
    deadline: Option<chrono::DateTime<chrono::Utc>>,
    reward: i64,
    /// 非空 = 累计口径（tier 等级系列）；空 = 增量口径
    tier: Option<String>,
    base_uploaded: i64,
    base_downloaded: i64,
    base_seed_seconds: i64,
    base_uploads: i64,
    metric: serde_json::Value,
    /// 认领之后新增的字幕数（与结算同窗）
    sub_since: i64,
}

/// 我的认领记录（含进行中任务的实时进度）。
pub(super) async fn records(
    state: &std::sync::Arc<AppState>,
    uid: i64,
) -> DomainResult<Vec<serde_json::Value>> {
    let rows: Vec<MineRow> = sqlx::query_as(
        "SELECT t.id, t.name, c.status, c.claimed_at, c.settled_at, \
                (c.claimed_at + (t.duration_days || ' days')::interval) \
                  AS deadline, \
                t.reward, t.tier, c.base_uploaded, c.base_downloaded, \
                c.base_seed_seconds, c.base_uploads, t.metric, \
                (SELECT count(*) FROM subtitles sub WHERE sub.user_id = \
                 c.user_id AND sub.deleted_at IS NULL \
                 AND sub.created_at >= c.claimed_at) AS sub_since \
         FROM task_claims c JOIN tasks t ON t.id = c.task_id \
         WHERE c.user_id = $1 ORDER BY c.id DESC LIMIT 20",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 指标现值一次性取齐（与 task_settle 同源；my_exams 暂无下载源）
    let stats: Option<(i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT u.uploaded, u.downloaded, \
                COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s \
                          WHERE s.user_id = u.id), 0)::bigint, \
                (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id \
                 AND t.approval_status = 1), \
                (SELECT count(*) FROM subtitles sub WHERE sub.user_id = \
                 u.id AND sub.deleted_at IS NULL) \
         FROM users u WHERE u.id = $1",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (cur_up, cur_dl, cur_seed, cur_uploads, cur_subtitles) =
        stats.unwrap_or((0, 0, 0, 0, 0));

    Ok(rows
        .iter()
        .map(|r| {
            let (up, dl, seed, ups, subs) = if r.tier.is_some() {
                (
                    cur_up,
                    cur_dl,
                    cur_seed,
                    cur_uploads,
                    cur_subtitles,
                )
            } else {
                (
                    cur_up - r.base_uploaded,
                    cur_dl - r.base_downloaded,
                    cur_seed - r.base_seed_seconds,
                    cur_uploads - r.base_uploads,
                    r.sub_since,
                )
            };
            serde_json::json!({
                "task_id": r.id, "name": r.name, "status": r.status,
                "claimed_at": r.claimed_at.to_rfc3339(),
                "settled_at": r.settled_at.map(|s| s.to_rfc3339()),
                "deadline": r.deadline.map(|d| d.to_rfc3339()),
                "reward": r.reward,
                "metric": r.metric,
                "current": {
                    "uploaded": up, "downloaded": dl,
                    "seed_seconds": seed,
                    "uploads": ups, "subtitles": subs,
                },
            })
        })
        .collect())
}
