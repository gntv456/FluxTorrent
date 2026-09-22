//! 字幕治理动作（0146/0148）：评分 + 坏字幕举报。
//! 元数据编辑/软删在 subtitles_meta.rs（300 行门禁按域拆分）。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::subtitles_util::subtitle_bad_threshold;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 评分 1-10，一人一条（主键约束）；重复投票覆盖原分数（A8：
/// rating_count 不增加——UPSERT 后重算聚合）
#[derive(Deserialize)]
struct VoteReq {
    score: i16,
}

#[post("/subtitles/{id}/vote")]
pub(super) async fn subtitle_vote(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<VoteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let sid = path.into_inner();
    if !(1..=10).contains(&body.score) {
        return Err(DomainError::Validation("评分需在 1-10 之间".into()));
    }
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM subtitles WHERE id = $1 AND deleted_at \
         IS NULL AND status = 1)",
    )
    .bind(sid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !exists {
        return Err(DomainError::NotFound(sid));
    }
    sqlx::query(
        "INSERT INTO subtitle_votes (subtitle_id, user_id, score) VALUES \
         ($1, $2, $3) ON CONFLICT (subtitle_id, user_id) DO UPDATE SET score \
         = EXCLUDED.score",
    )
    .bind(sid)
    .bind(auth.id)
    .bind(body.score)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 聚合列重算（覆盖投票时 sum 变 / count 不变，A8 断言口径）
    sqlx::query(
        "UPDATE subtitles s SET rating_sum = v.sum, rating_count = v.cnt \
         FROM (SELECT COALESCE(sum(score),0) AS sum, count(*) AS cnt FROM \
         subtitle_votes WHERE subtitle_id = $1) v WHERE s.id = $1",
    )
    .bind(sid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": sid, "score": body.score })))
}

/// 标记坏字幕：bad_reports++，达阈值自动隐藏（status 置 2 = rejected）。
/// 同一用户对同一字幕只计一次（reports 表未结案举报去重）。
#[post("/subtitles/{id}/report")]
pub(super) async fn subtitle_report(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let sid = path.into_inner();
    // 去重：已有未处理举报不再累计（reports 表 status IN (0,2)）
    let dup: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM reports WHERE ref_type = 'subtitle' \
         AND ref_id = $1 AND reporter_id = $2 AND status IN (0, 2))",
    )
    .bind(sid)
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if dup {
        return Ok(ok(serde_json::json!({
            "id": sid, "already": true,
            "msg": "你已标记过该字幕",
        })));
    }
    sqlx::query(
        "INSERT INTO reports (reporter_id, ref_type, ref_id, reason) VALUES \
         ($1, 'subtitle', $2, '坏字幕标记（自动）')",
    )
    .bind(auth.id)
    .bind(sid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let n = sqlx::query(
        "UPDATE subtitles SET bad_reports = bad_reports + 1 WHERE id = $1 \
         AND deleted_at IS NULL",
    )
    .bind(sid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(sid));
    }
    let threshold = subtitle_bad_threshold(&state.repo.db).await?;
    let mut hidden = false;
    if threshold > 0 {
        let n = sqlx::query(
            "UPDATE subtitles SET status = 2, moderated_by = NULL, \
             moderated_at = now() WHERE id = $1 AND deleted_at IS NULL AND \
             bad_reports >= $2 AND status = 1",
        )
        .bind(sid)
        .bind(threshold)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
        hidden = n > 0;
    }
    Ok(ok(serde_json::json!({
        "id": sid, "hidden": hidden, "threshold": threshold,
    })))
}
