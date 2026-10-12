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
    // C3：投票门槛——月度评选按评分加权，新号/低等级批量刷 10 分可捧私账。
    // staff 放行；其余要求注册满 7 天且等级 ≥1（Peasant 不可投）。
    if auth.class_id < 90 {
        let qualified: bool = sqlx::query_scalar(
            "SELECT (class_id >= 1 AND created_at < now() - interval '7 \"
             days') FROM users WHERE id = $1",
        )
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        if !qualified {
            return Err(DomainError::Validation(
                "注册满 7 天且通过新手等级后才能为字幕评分".into(),
            ));
        }
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
    // 审计修复（P-131，批三十八实测）：旧写法三条语句各跑各的——
    // ① EXISTS 去重 → ② INSERT reports（**立即提交**）→ ③ UPDATE subtitles
    // 带 `deleted_at IS NULL`，0 行才回 404。于是对一条**已软删**的字幕点举报
    // 会「报 404 + 已经落了一张处置不了的孤儿举报单」（实测 rows 1→2），
    // 版主队列里全是这种点进去只会 500/空白的死单；两个人同时点还会双双越过
    // EXISTS、第二个撞部分唯一索引 uq_reports_open_per_reporter 变成 500。
    // 现在一把事务：先锁「还活着的字幕」，没有就直接 404（事务里没写过任何东西），
    // 有就靠唯一索引去重——举报单与 bad_reports 同生同死。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let alive: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM subtitles WHERE id = $1 AND deleted_at IS NULL \
         FOR UPDATE",
    )
    .bind(sid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if alive.is_none() {
        return Err(DomainError::NotFound(sid));
    }
    let fresh: Option<i64> = sqlx::query_scalar(
        "INSERT INTO reports (reporter_id, ref_type, ref_id, reason) VALUES \
         ($1, 'subtitle', $2, '坏字幕标记（自动）') \
         ON CONFLICT (ref_type, ref_id, reporter_id) WHERE status IN (0, 2) \
         DO NOTHING RETURNING id",
    )
    .bind(auth.id)
    .bind(sid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let already = fresh.is_none();
    if already {
        // 已有在处理中的同款举报：本次不累计计数，但要把刚拿到的锁放掉
        tx.commit()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        return Ok(ok(serde_json::json!({
            "id": sid, "already": true,
            "msg": "你已标记过该字幕",
        })));
    }
    sqlx::query(
        "UPDATE subtitles SET bad_reports = bad_reports + 1 WHERE id = $1",
    )
    .bind(sid)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
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
        "id": sid, "already": false, "hidden": hidden,
        "threshold": threshold,
    })))
}
