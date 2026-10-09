//! CUE 表合规判定端点（G3 后半）。
//!
//! 判据实现在 `logcheck::cue`（纯函数 + 单测）；本模块只做取数与出参：
//! 传入 `cue` 正文则直接判，缺省读该种 `kind='cue'` 的工件正文
//! （发种时经 artifact 通道上传的 .cue 内容）。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
pub(crate) struct CueCheckReq {
    /// 直接传 CUE 正文；缺省则读该种 `kind='cue'` 的工件正文
    #[serde(default)]
    cue: Option<String>,
}

/// 判定该种的 CUE 表是否合规（HasCue 的合规性，非「有没有文件」）。
#[post("/torrents/{id}/cue-check")]
pub async fn cue_check(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<CueCheckReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    // 子资源准入门（0288 口径）：CUE 判定结果对不可见种不提供
    crate::torrents::assert_visible(
        &state.repo.db,
        id,
        (auth.id, auth.class_id >= 90),
    )
    .await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrents \
                            WHERE id = $1)",
    )
    .bind(id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !exists {
        return Err(DomainError::NotFound(id));
    }
    // 注意：`Some("")` 与 `None` 语义不同——前者是「明确传了一张空表」，
    // 应判为不合规；后者才是「没传，回落读工件」。故此处不做 trim 过滤。
    let text = match body.cue.as_deref() {
        Some(t) => t.to_string(),
        None => {
            let t: Option<String> = sqlx::query_scalar(
                "SELECT body FROM torrent_artifacts \
                 WHERE torrent_id = $1 AND kind = 'cue' ORDER BY id LIMIT 1",
            )
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            t.ok_or_else(|| {
                DomainError::Validation(
                    "未提供 CUE 正文，且该种未挂 kind='cue' 的工件".into(),
                )
            })?
        }
    };
    let c = crate::logcheck::cue::parse(&text);
    Ok(ok(serde_json::json!({
        "valid": c.valid,
        "tracks": c.tracks,
        "has_file": c.has_file,
        "missing_index": c.missing_index,
        "issues": c.issues,
    })))
}
