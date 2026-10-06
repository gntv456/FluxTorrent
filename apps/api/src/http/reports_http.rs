//! 举报受理（0285 从 misc_handlers.rs 拆出）。
//!
//! 拆分理由：`misc_handlers.rs` 恰好顶在 300 行门禁上限，举报去重这类
//! 「必须新增分支」的改动无处落笔——按门禁口径「拆文件」而非放宽基线。

use actix_web::{post, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::auth_infra::require_auth;

#[derive(Deserialize)]
struct ReportReq {
    ref_type: String,
    ref_id: i64,
    reason: String,
}

#[post("/reports")]
pub async fn report_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ReportReq>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let allowed = ["torrent", "comment", "user", "subtitle", "forum"];
    if !allowed.contains(&body.ref_type.as_str()) {
        return Err(DomainError::Validation("非法的举报对象".into()));
    }
    if body.reason.trim().is_empty() || body.reason.len() > 500 {
        return Err(DomainError::Validation("举报理由需 1-500 字".into()));
    }
    // 目标存在性校验（此前任意 ref_id 含不存在对象可无限提交）
    let target_table = match body.ref_type.as_str() {
        "torrent" => Some(("torrents", "approval_status = 1")),
        "comment" => Some(("comments", "true")),
        "user" => Some(("users", "status < 2")),
        "subtitle" => Some(("subtitles", "true")),
        "forum" => Some(("topics", "true")),
        _ => None,
    };
    if let Some((table, extra)) = target_table {
        let sql = format!(
            "SELECT EXISTS(SELECT 1 FROM {table} WHERE id = $1 AND {extra})"
        );
        let exists: bool = sqlx::query_scalar(&sql)
            .bind(body.ref_id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
        if !exists {
            return Err(DomainError::NotFound(body.ref_id));
        }
    }
    // 去重（0285）：部分唯一索引 uq_reports_open_per_reporter 挡住「同一举报人
    // 对同一目标的多条在处理中」。此前无条件 INSERT，50 次连点＝队列 50 行，
    // 版主被迫逐条处置同一件事。
    let id: Option<i64> = sqlx::query_scalar(
        "INSERT INTO reports (reporter_id, ref_type, ref_id, reason) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (ref_type, ref_id, reporter_id) WHERE status IN (0, 2) \
         DO NOTHING RETURNING id",
    )
    .bind(auth.id)
    .bind(&body.ref_type)
    .bind(body.ref_id)
    .bind(body.reason.trim())
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "id": id,
        "duplicate": id.is_none(),
    })))
}
