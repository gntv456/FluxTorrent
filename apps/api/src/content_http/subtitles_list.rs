//! 字幕列表与下载（0146：分页信封、真实 size、评分、匿名脱敏、扩展名修正）。
//! 上传链路在 subtitles.rs；元数据治理在 subtitles_meta.rs。

use actix_web::{get, web, HttpRequest, HttpResponse, Responder};

use super::subtitles_util::subtitle_bad_threshold;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(sqlx::FromRow, serde::Serialize)]
struct SubtitleRow {
    id: i64,
    torrent_id: Option<i64>,
    username: Option<String>,
    title: String,
    lang: Option<String>,
    lang_id: Option<i16>,
    downloads: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    /// 文件大小（字节；0146 起真实落库，历史行回填）
    size: i64,
    ext: Option<String>,
    anon: bool,
    rating_sum: i32,
    rating_count: i32,
}

/// 字幕列表（包子站 subtitles.php 口径 + 0146）：
/// search/lang/letter/torrent_id 筛选 + page/per_page/sort/order 分页排序。
/// 只出未删且过审（或免审）的行；坏字幕达阈值自动隐藏。
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
    let torrent_id: Option<i64> = q
        .get("torrent_id")
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|v| *v > 0);
    let page = q
        .get("page")
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(1);
    let per_page = q
        .get("per_page")
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|v| [25, 50, 100].contains(v))
        .unwrap_or(50);
    let order = if q.get("order").map(String::as_str) == Some("asc") {
        "ASC"
    } else {
        "DESC"
    };
    let sort = match q.get("sort").map(String::as_str) {
        Some("downloads") => "s.downloads".to_string(),
        Some("size") => "s.size".to_string(),
        Some("rating") => {
            "(s.rating_sum::float / NULLIF(s.rating_count,0))".to_string()
        }
        // P1-4 匹配分排序（OpenSubtitles 加权收敛版）：torrent 命中 100 >
        // release_name 归一化相等 80 > verified 20 > 下载数兜底。IMDB+语言
        // 需 media_info->>'imdb_id' 数据源，全站尚无该录入链路，落地后在此
        // 追加 +50 档（方案 §5 第 3 条）。
        Some("match") => {
            let sub = sqlx::query_scalar::<_, Option<String>>(
                "SELECT lower(regexp_replace(name, '[^a-zA-Z0-9]', '', 'g')) \
                 FROM torrents WHERE id = $1",
            )
            .bind(torrent_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
            match (torrent_id, sub) {
                (Some(tid), Some(sub)) => format!(
                    "(CASE WHEN s.torrent_id = {tid} THEN 100 \
                     WHEN s.release_name = '{sub}' THEN 80 \
                     ELSE 0 END + CASE WHEN s.verified THEN 20 ELSE 0 END \
                     + s.downloads)"
                ),
                (Some(tid), None) => format!(
                    "(CASE WHEN s.torrent_id = {tid} THEN 100 ELSE 0 END \
                     + CASE WHEN s.verified THEN 20 ELSE 0 END + s.downloads)"
                ),
                _ => "(CASE WHEN s.verified THEN 20 ELSE 0 END + s.downloads)"
                    .to_string(),
            }
        }
        _ => "s.created_at".to_string(),
    };
    let threshold = subtitle_bad_threshold(&state.repo.db).await?;
    // 谓词只拼一份：count 与列表同谓词（A7 验收点）
    let predicates = "\
         s.deleted_at IS NULL AND s.status = 1 \
         AND (s.bad_reports < $4 OR $4 <= 0) \
         AND s.title ILIKE $1 \
         AND ($2::text IS NULL OR s.lang = $2) \
         AND ($3::text IS NULL OR s.title ILIKE $3 || '%') \
         AND ($5::bigint IS NULL OR s.torrent_id = $5)";
    let total: i64 = sqlx::query_scalar(
        &format!("SELECT count(*) FROM subtitles s WHERE {predicates}"),
    )
    .bind(&search)
    .bind(&lang)
    .bind(&letter)
    .bind(threshold)
    .bind(torrent_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let rows: Vec<SubtitleRow> = sqlx::query_as(
        &format!(
            "SELECT s.id, s.torrent_id, u.username, s.title, s.lang, \
             s.lang_id, s.downloads, s.created_at, s.size, s.ext, s.anon, \
             s.rating_sum, s.rating_count \
             FROM subtitles s LEFT JOIN users u ON u.id = s.user_id \
             WHERE {predicates} ORDER BY {sort} {order}, s.id DESC \
             OFFSET $6 LIMIT $7"
        ),
    )
    .bind(&search)
    .bind(&lang)
    .bind(&letter)
    .bind(threshold)
    .bind(torrent_id)
    .bind((page - 1) * per_page)
    .bind(per_page)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 匿名上传：列表口径直接脱敏——anon 行不回真实用户名
    let rows: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|mut r| {
            let name =
                if r.anon { None } else { r.username.take() };
            let rating = if r.rating_count > 0 {
                // 1-10 均分保留一位小数（sum/count 四舍五入到 0.1）
                serde_json::json!(
                    (r.rating_sum as f64 / r.rating_count as f64 * 10.0)
                        .round()
                        / 10.0
                )
            } else {
                serde_json::Value::Null
            };
            serde_json::json!({
                "id": r.id, "torrent_id": r.torrent_id, "username": name,
                "title": r.title, "lang": r.lang, "lang_id": r.lang_id,
                "downloads": r.downloads, "created_at": r.created_at,
                "size": r.size, "ext": r.ext, "rating": rating,
                "rating_count": r.rating_count,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({
        "items": rows, "total": total, "page": page, "per_page": per_page,
    })))
}

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
    let row: Option<(
        String,
        Option<i64>,
        String,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
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
        let ext = sub_ext
            .filter(|e| !e.is_empty())
            .unwrap_or_else(|| match mime.as_str() {
                "application/pdf" => "pdf".into(),
                _ => "srt".into(),
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

