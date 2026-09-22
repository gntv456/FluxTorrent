//! 字幕下载与语言字典（0146/0148）：从 subtitles_list.rs 按域拆出
//! （300 行门禁；列表/筛选/AI 三态在 subtitles_list.rs）。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 字幕下载：计数 +1；本地附件直接回文件字节。0146：扩展名按 mime + 落库 ext
/// 决定（字幕格式直通，兜底 .srt 而非 .bin），文件名带语言代码。
#[get("/subtitles/{id}/download")]
pub(super) async fn subtitle_download(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let sid = path.into_inner();
    // (file_ref, torrent_id, title, lang, ext)
    type DlRow = (String, Option<i64>, String, Option<String>, Option<String>);
    let row: Option<DlRow> = sqlx::query_as(
        "SELECT file_ref, torrent_id, title, lang, ext FROM subtitles \
         WHERE id = $1 AND deleted_at IS NULL AND status = 1",
    )
    .bind(sid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((file_ref, torrent_id, title, lang, sub_ext)) = row else {
        return Err(DomainError::NotFound(sid));
    };
    sqlx::query("UPDATE subtitles SET downloads = downloads + 1 WHERE id = $1")
        .bind(sid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(sha) = file_ref.strip_prefix("attach://") {
        let row: Option<(String, i64)> = sqlx::query_as(
            "SELECT mime, size FROM attachments WHERE sha256 = $1",
        )
        .bind(sha)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let Some((mime, _size)) = row else {
            return Err(DomainError::NotFound(sid));
        };
        let bytes = crate::storage::get(&state.repo.db, sha)
            .await
            .ok_or(DomainError::NotFound(sid))?;
        let safe_title = safe_filename(&title);
        // 扩展名优先级：落库 ext（上传时按文件名判定）> mime 常见映射 > .srt
        let ext =
            sub_ext.filter(|e| !e.is_empty()).unwrap_or_else(|| {
                match mime.as_str() {
                    "application/pdf" => "pdf".into(),
                    _ => "srt".into(),
                }
            });
        let name = match lang.as_deref() {
            Some(code) if !code.is_empty() => {
                format!("{safe_title}.{code}.{ext}")
            }
            _ => format!("{safe_title}.{ext}"),
        };
        return Ok(HttpResponse::Ok()
            .content_type(mime)
            .insert_header((
                actix_web::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ))
            .body(bytes));
    }
    let _ = auth;
    Ok(ok(serde_json::json!({
        "id": sid,
        "title": title,
        "torrent_id": torrent_id,
        "file_ref": file_ref,
    })))
}

/// 下载文件名清洗（与旧链路口径一致：字母数字 + 常见标点，截 80 字符）
fn safe_filename(title: &str) -> String {
    title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " ._-()[]（）【】".contains(c) {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect::<String>()
}

// ============ 语言字典（P1-2） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct SubtitleLangRow {
    id: i16,
    code: String,
    name: String,
    flag: Option<String>,
    position: i32,
}

/// 语言字典（公开；替代前端 31 项硬编码）
#[get("/subtitles/langs")]
pub(super) async fn subtitle_langs(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl actix_web::Responder> {
    let rows: Vec<SubtitleLangRow> = sqlx::query_as(
        "SELECT id, code, name, flag, position FROM subtitle_langs ORDER \
         BY position, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}
