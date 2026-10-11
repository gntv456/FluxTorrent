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
pub(crate) async fn visibility_denied(
    db: &sqlx::PgPool,
    uid: i64,
    sha: &str,
) -> DomainResult<Option<bool>> {
    // P-57（批二十九实测）：同一 sha 现在可以有多行（0347 起按人隔离权限
    // 行、物理文件共享），所以这里必须是**聚合**判据，不能拿「任意一行」的
    // visibility 说话——两行（A private + B shared）时旧写法取到哪条全凭
    // 数据库心情，同一张图可能对某些人 404、对某些人全站可读。
    // 放行 = staff（class≥90）∥ 自己有行 ∥ 存在 shared 行；
    // 等级比较放在 SQL 里做，也顺手绕开「i64 解码 INT4」那个老坑。
    let (has_row, visible): (bool, bool) = sqlx::query_as(
        "SELECT (SELECT EXISTS(SELECT 1 FROM attachments WHERE sha256 = $2)), \
                COALESCE((SELECT u.class_id >= 90 FROM users u \
                          WHERE u.id = $1), FALSE) \
             OR EXISTS (SELECT 1 FROM attachments m WHERE m.sha256 = $2 \
                        AND m.user_id = $1) \
             OR EXISTS (SELECT 1 FROM attachments s WHERE s.sha256 = $2 \
                        AND s.visibility = 'shared')",
    )
    .bind(uid)
    .bind(sha)
    .fetch_one(db)
    .await
    .map_err(internal)?;
    // 库里没有这份内容时不算「拒绝」：交给调用方按 404 处理（与旧口径一致）
    Ok(if has_row { Some(!visible) } else { None })
}

/// 附件读取限流（P2，2026-10-06 安全审计）：读取是「S3/本地盘 IO + DB 查询」
/// 路径，登录用户可循环打。60 次/分钟（页面一次加载几十张缩略图远够）。
/// P-61：这个数字以前只写在注释里——实际走的是登录档 5 次/分，一页第 6 张
/// 图就 429。现在显式传 60。
async fn read_throttled(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
) -> DomainResult<()> {
    crate::http::throttle_limited(&state, format!("att:{}", uid), 60).await
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
    // P-57：同 sha 可能有多行权限行，mime/size 由内容决定 ⇒ 任取一行等价，
    // 但必须显式 LIMIT 1，否则 fetch_optional 遇多行直接报错。
    let row: Option<(String, i64)> = sqlx::query_as(
        "SELECT mime, size FROM attachments WHERE sha256 = $1 \
         ORDER BY id LIMIT 1",
    )
    .bind(&sha)
    .fetch_optional(&state.repo.db)
            .await
            .map_err(internal)?;
    let Some((mime, size)) = row else {
        return Err(DomainError::NotFound(0));
    };
    // P-60（批二十九）：旧实现为「怕 actix 把 HEAD 的 Content-Length 归零」
    // 挂了一份与文件等长的 0 占位 body——HEAD 只要头不要体，200MiB 的视频
    // 每预检一次就整块分配一次（限流只是把放大器关小，没关）。
    // 现在只显式插 Content-Length，AT 闸门钉 HEAD 回来的是真实长度。
    let mut resp = HttpResponse::Ok()
        .content_type(mime)
        .insert_header(("Accept-Ranges", "bytes"))
        .insert_header((
            actix_web::http::header::CACHE_CONTROL,
            "private, max-age=31536000, immutable",
        ))
        .finish();
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
    // P-57：同 sha 可能有多行权限行，mime/size 由内容决定 ⇒ 任取一行等价，
    // 但必须显式 LIMIT 1，否则 fetch_optional 遇多行直接报错。
    let row: Option<(String, i64)> = sqlx::query_as(
        "SELECT mime, size FROM attachments WHERE sha256 = $1 \
         ORDER BY id LIMIT 1",
    )
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
