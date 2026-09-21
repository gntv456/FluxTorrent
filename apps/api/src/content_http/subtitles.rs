use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[post("/subtitles")]
pub(super) async fn subtitle_upload(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SubtitleUploadReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.title.trim().is_empty() {
        return Err(DomainError::Validation("字幕标题不能为空".into()));
    }
    // torrent_id=0 或不存在的种子 → 存 NULL（外键可空），不阻断独立字幕分享
    let torrent_id = if body.torrent_id > 0 {
        Some(body.torrent_id)
    } else {
        None
    };
    let file_ref =
        if let Some(sha) = body.file_sha.as_deref().map(str::trim).filter(|s| {
            s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
        }) {
            // 校验附件归属：必须是本人在 attachments 上传过的文件（防冒用他人 sha）
            let owned: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM attachments WHERE sha256 \
             = $1 AND user_id = $2)",
            )
            .bind(sha)
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
            if !owned {
                return Err(DomainError::Validation(
                    "附件未上传或不存在（请先通过上传接口提交字幕文件）".into(),
                ));
            }
            format!("attach://{sha}")
        } else if let Some(ext) =
            body.file_ref.as_deref().map(str::trim).filter(|s| {
                s.starts_with("http://") || s.starts_with("https://")
            })
        {
            ext.to_string()
        } else {
            return Err(DomainError::Validation(
                "请上传字幕文件（或提供 http/https 直链）".into(),
            ));
        };
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO subtitles (torrent_id, user_id, title, lang, \
         file_ref) VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(torrent_id)
    .bind(auth.id)
    .bind(&body.title)
    .bind(&body.lang)
    .bind(&file_ref)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 发字幕 +5 火花（旧站口径）
    let idem = format!("subtitle:{}:{}", auth.id, id);
    earn_spark(&state.repo.db, auth.id, 5, "subtitle", &idem).await?;
    Ok(ok(serde_json::json!({ "id": id, "reward": 5 })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct SubtitleRow {
    id: i64,
    torrent_id: Option<i64>,
    username: Option<String>,
    title: String,
    lang: Option<String>,
    downloads: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    /// 文件大小（字节；无真实文件时为 0）
    #[sqlx(default)]
    size: Option<i64>,
}

/// 字幕列表（包子站 subtitles.php 口径）：search 关键词 + lang 语言 + letter 首字母筛选
#[get("/subtitles")]
pub(super) async fn subtitle_list(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<impl Responder> {
    let search = q
        .get("search")
        .map(|s| crate::http::like_pattern(&s))
        .unwrap_or_else(|| "%".into());
    let lang = q.get("lang_id").filter(|s| s.as_str() != "0").cloned();
    let letter = q.get("letter").filter(|s| !s.is_empty()).cloned();
    let rows = sqlx::query_as::<_, SubtitleRow>(
        "SELECT s.id, s.torrent_id, u.username, s.title, s.lang, s.downloads, s.created_at, 0::bigint AS size \
         FROM subtitles s LEFT JOIN users u ON u.id = s.user_id \
         WHERE s.title ILIKE $1 \
           AND ($2::text IS NULL OR s.lang = $2) \
           AND ($3::text IS NULL OR s.title ILIKE $3 || '%') \
         ORDER BY s.id DESC LIMIT 50",
    )
    .bind(search)
    .bind(lang)
    .bind(letter)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 字幕下载：计数 +1；本地附件（attach://sha）直接回文件字节（Content-Disposition
/// 按 title 命名 .txt/.ass/.srt 兜底），外部直链返回 JSON 引用由前端跳转。
/// 审计修复（P1 空壳链路）：旧版只回 file_ref JSON——下载按钮点开是引用文本而非文件。
#[get("/subtitles/{id}/download")]
pub(super) async fn subtitle_download(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let sid = path.into_inner();
    let row: Option<(String, Option<i64>, String)> = sqlx::query_as(
        "SELECT file_ref, torrent_id, title FROM subtitles WHERE id = $1",
    )
    .bind(sid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((file_ref, torrent_id, title)) = row else {
        return Err(DomainError::NotFound(sid));
    };
    sqlx::query("UPDATE subtitles SET downloads = downloads + 1 WHERE id = $1")
        .bind(sid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(sha) = file_ref.strip_prefix("attach://") {
        // 与 /attachments/{sha} 同源读取（本地卷内容寻址），但不经 302：直接回字节，
        // 便于客户端「点开即存文件」；文件名用字幕标题（清洗非法字符）。
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
        let safe_title: String = title
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || " ._-()[]（）【】".contains(c) {
                    c
                } else {
                    '_'
                }
            })
            .take(80)
            .collect::<String>();
        let ext = if mime == "application/pdf" {
            "pdf"
        } else if mime == "text/plain" {
            "txt"
        } else {
            "bin"
        };
        return Ok(HttpResponse::Ok()
            .content_type(mime)
            .insert_header((
                actix_web::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{safe_title}.{ext}\""),
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

// ============ M18 课本中心 ============

#[derive(Deserialize)]
struct SubtitleUploadReq {
    torrent_id: i64,
    title: String,
    #[serde(default)]
    lang: Option<String>,
    /// 真实文件的附件 sha256（前端先调 POST /attachments 上传拿到）。
    /// 审计修复（P1 空壳链路）：旧版 file_ref 是客户端任意字符串或后端伪造的
    /// s3:// UUID——下载只回 JSON 引用，全链路无文件本体。现统一走 attachments
    /// 存储（本地 savedirectory 卷 + sha256 内容寻址 + 配额），file_ref 记
    /// attach://<sha>；历史外部引用（http(s):// 链接）仍按原样展示。
    #[serde(default)]
    file_sha: Option<String>,
    /// 兼容字段：外部字幕站直链（http/https），与本地附件二选一
    #[serde(default)]
    file_ref: Option<String>,
}
