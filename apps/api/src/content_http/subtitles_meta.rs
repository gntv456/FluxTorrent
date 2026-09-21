//! 字幕区治理端点（0146 P1）：编辑/软删/评分/坏字幕标记/语言字典。
//! 上传链路在 subtitles.rs；列表下载在 subtitles_list.rs；求字幕悬赏在
//! subtitles_requests.rs（300 行门禁按域拆分）。

use actix_web::{delete, patch, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::subtitles_util::{subtitle_bad_threshold, trim_opt};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 本人或 staff（class ≥ 90）可操作
async fn can_modify(
    auth: &crate::http::AuthUser,
    sub_user_id: i64,
) -> bool {
    auth.id == sub_user_id || auth.class_id >= 90
}

// ============ 编辑 / 软删（P1-6） ============

#[derive(Deserialize, Default)]
struct SubtitlePatchReq {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    lang: Option<String>,
    #[serde(default)]
    fps: Option<f64>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    producer: Option<String>,
    #[serde(default)]
    proofreader: Option<String>,
    #[serde(default)]
    author_name: Option<String>,
    #[serde(default)]
    anon: Option<bool>,
    #[serde(default)]
    verified: Option<bool>,
}

/// 编辑字幕元数据（本人或 modo；verified 仅 staff 可改）
#[patch("/subtitles/{id}")]
pub(super) async fn subtitle_patch(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<SubtitlePatchReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let sid = path.into_inner();
    let owner: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM subtitles WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(sid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(sid));
    };
    if !can_modify(&auth, owner).await {
        return Err(DomainError::Forbidden);
    }
    if let Some(fps) = body.fps {
        if !(0.0..=240.0).contains(&fps) {
            return Err(DomainError::Validation(
                "FPS 需在 0-240 之间".into(),
            ));
        }
    }
    let lang_id: Option<i16> = match body.lang.as_deref().map(str::trim) {
        Some(code) if !code.is_empty() && code != "0" => {
            sqlx::query_scalar::<_, i16>(
                "SELECT id FROM subtitle_langs WHERE code = $1",
            )
            .bind(code)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
        }
        _ => None,
    };
    // verified/审核字段仅 staff 可写
    let verified_set = match body.verified {
        Some(_) if auth.class_id >= 90 => true,
        Some(_) => return Err(DomainError::Forbidden),
        None => false,
    };
    // 标题可编辑（0147 行内编辑入口）：空串视为不改（COALESCE 语义）
    let title = trim_opt(&body.title);
    if body.title.as_deref().map(str::trim) == Some("") {
        return Err(DomainError::Validation("标题不能为空".into()));
    }
    let sql = if verified_set {
        "UPDATE subtitles SET title = COALESCE($2, title), lang = \
         COALESCE($3, lang), lang_id = COALESCE($4, lang_id), fps = \
         COALESCE($5, fps), source = COALESCE($6, source), producer = \
         COALESCE($7, producer), proofreader = COALESCE($8, proofreader), \
         author_name = COALESCE($9, author_name), anon = COALESCE($10, \
         anon), verified = $11 WHERE id = $1 AND deleted_at IS NULL"
    } else {
        "UPDATE subtitles SET title = COALESCE($2, title), lang = \
         COALESCE($3, lang), lang_id = COALESCE($4, lang_id), fps = \
         COALESCE($5, fps), source = COALESCE($6, source), producer = \
         COALESCE($7, producer), proofreader = COALESCE($8, proofreader), \
         author_name = COALESCE($9, author_name), anon = COALESCE($10, \
         anon) WHERE id = $1 AND deleted_at IS NULL"
    };
    let n = sqlx::query(sql)
        .bind(sid)
        .bind(title)
        .bind(trim_opt(&body.lang))
        .bind(lang_id)
        .bind(body.fps)
        .bind(trim_opt(&body.source))
        .bind(trim_opt(&body.producer))
        .bind(trim_opt(&body.proofreader))
        .bind(trim_opt(&body.author_name))
        .bind(body.anon)
        .bind(body.verified.unwrap_or(false))
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if n.rows_affected() == 0 {
        return Err(DomainError::NotFound(sid));
    }
    state
        .repo
        .audit(Some(auth.id), "subtitle.patch", Some(sid))
        .await;
    Ok(ok(serde_json::json!({ "id": sid })))
}

/// 软删（本人或 modo；保留 attachments 引用，0146 P1-6）
#[delete("/subtitles/{id}")]
pub(super) async fn subtitle_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let sid = path.into_inner();
    let owner: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM subtitles WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(sid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(sid));
    };
    if !can_modify(&auth, owner).await {
        return Err(DomainError::Forbidden);
    }
    let n = sqlx::query(
        "UPDATE subtitles SET deleted_at = now() WHERE id = $1 AND \
         deleted_at IS NULL",
    )
    .bind(sid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(sid));
    }
    state
        .repo
        .audit(Some(auth.id), "subtitle.delete", Some(sid))
        .await;
    Ok(ok(serde_json::json!({ "id": sid, "deleted": true })))
}

// ============ 评分 / 坏字幕标记（P1-5） ============

#[derive(Deserialize)]
struct VoteReq {
    score: i32,
}

/// 评分 1-10，一人一条（主键约束）；重复投票覆盖原分数（A8：
/// rating_count 不增加——UPSERT 后重算聚合）
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
