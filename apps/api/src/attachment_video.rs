//! 论坛视频内嵌 V2（0190）：读取端（Range/206 + HEAD）。
//! 上传端（魔数/配额/频率/落库）在 attachment_video_upload.rs。
//!
//! Range 命中 → 206 + Content-Range + Accept-Ranges（<video> 拖进度条）；
//! 无 Range 头维持 200 全量语义（GET /attachments/{sha} 原图片路径共用，
//! 行为不变）；需登录（防匿名爬附件占带宽）。

use actix_web::{head, web, HttpRequest, HttpResponse};

use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

/// Range 解析：`bytes=start-[end]`（单区间；后缀区间不支持→200 全量）。
fn parse_range(h: &str, total: i64) -> Option<(u64, u64)> {
    let spec = h.strip_prefix("bytes=")?;
    let (s, e) = spec.split_once('-')?;
    if s.is_empty() {
        return None; // 后缀区间（bytes=-N）：视频拖动用不到，回落全量
    }
    let start: u64 = s.parse().ok()?;
    if start as i64 >= total {
        return None;
    }
    let end = if e.is_empty() {
        total as u64 - 1
    } else {
        e.parse::<u64>().ok()?.min(total as u64 - 1)
    };
    if end < start {
        return None;
    }
    Some((start, end))
}

/// 附件读取的可见性判定（0294 / 安全审计 P1-2）：
/// shared=任何成员（图床/帖子引用语义）；private=上传者本人或 staff；
/// staff=仅 staff。返回 None 表示无权（对外统一 404，不泄存在性）。
async fn visibility_denied(
    db: &sqlx::PgPool,
    uid: i64,
    sha: &str,
) -> DomainResult<Option<bool>> {
    // class_id 顺带取：staff 读 private/staff 附件均放行。
    // users.class_id 是 INT4，必须按 i32 解码（二轮审计线上验证发现：
    // i64 解码 INT4 报 mismatched types → 他人读取 private 附件 500）
    let row: Option<(String, i32)> = sqlx::query_as(
        "SELECT a.visibility, u.class_id FROM attachments a \
         JOIN users u ON u.id = $1 WHERE a.sha256 = $2 AND a.user_id <> $1",
    )
    .bind(uid)
    .bind(sha)
    .fetch_optional(db)
    .await
    .map_err(internal)?;
    Ok(row.map(|(vis, class)| {
        // 他人行：private 仅 staff 可读；staff 档同样仅 staff；shared 放行
        vis != "shared" && class < 90
    }))
}

/// 附件读取限流（P2，2026-10-06 安全审计）：读取是「S3/本地盘 IO + DB 查询」
/// 路径，登录用户可循环打。60 次/分钟（页面一次加载几十张缩略图远够）。
async fn read_throttled(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
) -> DomainResult<()> {
    crate::http::throttle(state, format!("att:{}", uid)).await
}

/// HEAD：预检长度与可区间性（<video> 元数据预载依赖）。
#[head("/attachments/{sha}")]
pub async fn head_attachment(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 二轮审计：GET 有 60/min 限流而 HEAD 没有——每请求一次 DB 双查询 +
    // size 字节占位分配，可被无界打；与 GET 同桶限流
    read_throttled(&state, auth.id).await?;
    let sha = path.into_inner();
    if sha.len() != 64 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(DomainError::Validation("sha256 格式无效".into()));
    }
    if visibility_denied(&state.repo.db, auth.id, &sha)
        .await?
        .unwrap_or(false)
    {
        return Err(DomainError::NotFound(0));
    }
    let row: Option<(String, i64)> =
        sqlx::query_as("SELECT mime, size FROM attachments WHERE sha256 = $1")
            .bind(&sha)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(internal)?;
    let Some((mime, size)) = row else {
        return Err(DomainError::NotFound(0));
    };
    // actix 对 HEAD 会剥 body 并把 Content-Length 清零；视频元数据预载依赖
    // 真实长度 → 挂一个等长占位 body（actix HEAD 管线剥 body 时会保留其
    // Content-Length 头值，不再重算为 0）。
    let n = usize::try_from(size).unwrap_or(usize::MAX);
    let mut resp = HttpResponse::Ok()
        .content_type(mime)
        .insert_header(("Accept-Ranges", "bytes"))
        .insert_header((
            actix_web::http::header::CACHE_CONTROL,
            "private, max-age=31536000, immutable",
        ))
        .body(vec![0u8; n]);
    resp.headers_mut().insert(
        actix_web::http::header::CONTENT_LENGTH,
        actix_web::http::header::HeaderValue::from_str(&size.to_string())
            .expect("size 合法整数"),
    );
    Ok(resp)
}

/// 附件读取统一实现（Range/206）。GET /attachments/{sha} 原端点是薄壳。
pub async fn serve_attachment(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    read_throttled(&state, auth.id).await?;
    let sha = path.into_inner();
    if sha.len() != 64 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(DomainError::Validation("sha256 格式无效".into()));
    }
    if visibility_denied(&state.repo.db, auth.id, &sha)
        .await?
        .unwrap_or(false)
    {
        return Err(DomainError::NotFound(0));
    }
    let row: Option<(String, i64)> =
        sqlx::query_as("SELECT mime, size FROM attachments WHERE sha256 = $1")
            .bind(&sha)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(internal)?;
    let Some((mime, size)) = row else {
        return Err(DomainError::NotFound(0));
    };

    let range = req
        .headers()
        .get(actix_web::http::header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|h| parse_range(h, size));
    match range {
        Some((start, end)) => {
            let len = (end - start + 1) as usize;
            let bytes = crate::storage::get(&state.repo.db, &sha)
                .await
                .ok_or(DomainError::NotFound(0))?;
            let slice = bytes
                .get(start as usize..start as usize + len)
                .ok_or(DomainError::NotFound(0))?;
            Ok(HttpResponse::PartialContent()
                .content_type(mime)
                .insert_header((
                    actix_web::http::header::CACHE_CONTROL,
                    "private, max-age=31536000, immutable",
                ))
                .insert_header(("Accept-Ranges", "bytes"))
                .insert_header((
                    actix_web::http::header::CONTENT_RANGE,
                    format!("bytes {start}-{end}/{}", size),
                ))
                .body(slice.to_vec()))
        }
        None => {
            let bytes = crate::storage::get(&state.repo.db, &sha)
                .await
                .ok_or(DomainError::NotFound(0))?;
            Ok(HttpResponse::Ok()
                .content_type(mime)
                .insert_header((
                    actix_web::http::header::CACHE_CONTROL,
                    "private, max-age=31536000, immutable",
                ))
                .insert_header(("Accept-Ranges", "bytes"))
                .body(bytes))
        }
    }
}
