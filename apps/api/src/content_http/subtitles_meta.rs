//! 字幕区治理端点（0146 P1）：编辑/软删/评分/坏字幕标记/语言字典。
//! 上传链路在 subtitles.rs；列表下载在 subtitles_list.rs；求字幕悬赏在
//! subtitles_requests.rs（300 行门禁按域拆分）。

use actix_web::{delete, patch, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::subtitles_util::trim_opt;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 本人或 staff（class ≥ 90）可操作
async fn can_modify(auth: &crate::http::AuthUser, sub_user_id: i64) -> bool {
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
    /// 0148 C0：AI 生成标记可改（纠错「AI 冒充人工」举报用）
    #[serde(default)]
    machine_translated: Option<bool>,
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
            return Err(DomainError::Validation("FPS 需在 0-240 之间".into()));
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
         anon), verified = $11, machine_translated = COALESCE($12, \
         machine_translated) WHERE id = $1 AND deleted_at IS NULL"
    } else {
        "UPDATE subtitles SET title = COALESCE($2, title), lang = \
         COALESCE($3, lang), lang_id = COALESCE($4, lang_id), fps = \
         COALESCE($5, fps), source = COALESCE($6, source), producer = \
         COALESCE($7, producer), proofreader = COALESCE($8, proofreader), \
         author_name = COALESCE($9, author_name), anon = COALESCE($10, \
         anon), machine_translated = COALESCE($12, machine_translated) \
         WHERE id = $1 AND deleted_at IS NULL"
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
        .bind(body.machine_translated)
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
