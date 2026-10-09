//! 种子预览片段（0337）：试读 / 试听。
//!
//! 物理文件复用 `attachments`（sha256 指向 + 配额 + 可见性都归附件体系），
//! 本模块只管「种子 ↔ 附件」的关联与排序。
//! ⚠️ 字段名沿用附件体系的 `sha256`（历史契约），**算法实为 SHA3-256**
//! （见 `attachment_http.rs` 的 0285 P2 注记）；同为 64 hex，比对口径一致。
//! 上传动线：先 `POST /attachments` 拿 sha256，再在此关联。
//! 「本站型是否启用」由 `site_type_packs.features->>'preview'` 决定（0337），
//! 不 match site_type。

use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrent_http::trump::feature_on;

const KINDS: [&str; 2] = ["preview", "audio"];

#[derive(Deserialize)]
pub(crate) struct AddPreviewReq {
    sha256: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    filename: Option<String>,
}

/// 关联一个预览片段（作者或 staff）。
#[post("/torrents/{id}/previews")]
pub async fn preview_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<AddPreviewReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if !feature_on(&state, "preview").await {
        return Err(DomainError::Validation(
            "本站型未启用预览（试读/试听）能力".into(),
        ));
    }
    let id = path.into_inner();
    let owner: Option<i64> =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(id));
    };
    if owner != auth.id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let sha = body.sha256.trim().to_lowercase();
    if sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(DomainError::Validation("sha256 格式不正确".into()));
    }
    let kind = body.kind.as_deref().unwrap_or("preview");
    if !KINDS.contains(&kind) {
        return Err(DomainError::Validation("预览类型不合法".into()));
    }
    let att: Option<(String, String, i64)> = sqlx::query_as(
        "SELECT filename, mime, size FROM attachments \
         WHERE sha256 = $1 AND (user_id = $2 OR visibility = 'shared') \
         LIMIT 1",
    )
    .bind(&sha)
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((fname, mime, size)) = att else {
        return Err(DomainError::Validation("附件不存在或未对本站共享".into()));
    };
    let filename = body
        .filename
        .as_deref()
        .map(str::trim)
        .filter(|f| !f.is_empty())
        .unwrap_or(&fname)
        .to_string();
    sqlx::query(
        "INSERT INTO torrent_previews \
           (torrent_id, sha256, filename, mime, size_bytes, kind) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (torrent_id, sha256) DO UPDATE SET kind = EXCLUDED.kind",
    )
    .bind(id)
    .bind(&sha)
    .bind(&filename)
    .bind(&mime)
    .bind(size)
    .bind(kind)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "sha256": sha, "kind": kind })))
}

/// 某颗种子的预览清单（详情页「试读/试听」入口数据源）。
#[get("/torrents/{id}/previews")]
pub async fn preview_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    // 子资源准入门（0288 口径）：待审/被拒种的主详情 404 时，
    // 预览文件名不能从这里漏出去
    crate::torrents::assert_visible(
        &state.repo.db,
        id,
        (auth.id, auth.class_id >= 90),
    )
    .await?;
    let rows: Vec<(String, String, String, i64, String)> = sqlx::query_as(
        "SELECT sha256, filename, mime, size_bytes, kind \
         FROM torrent_previews WHERE torrent_id = $1 \
         ORDER BY sort, id LIMIT 50",
    )
    .bind(id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = rows
        .into_iter()
        .map(|(sha, filename, mime, size, kind)| {
            serde_json::json!({
                "sha256": sha, "filename": filename, "mime": mime,
                "size": size, "kind": kind,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({ "items": items })))
}

#[derive(Deserialize)]
pub(crate) struct DelPreviewReq {
    sha256: String,
}

/// 解除关联（作者或 staff）。物理文件仍归附件体系，不在此删除。
#[delete("/torrents/{id}/previews")]
pub async fn preview_remove(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<DelPreviewReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let owner: Option<i64> =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(id));
    };
    if owner != auth.id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let n = sqlx::query(
        "DELETE FROM torrent_previews WHERE torrent_id = $1 AND sha256 = $2",
    )
    .bind(id)
    .bind(body.sha256.trim().to_lowercase())
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    Ok(ok(serde_json::json!({ "removed": n })))
}
