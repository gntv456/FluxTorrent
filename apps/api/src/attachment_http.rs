//! 附件/图床（0100，NP Pictured 最小落地）。
//! 从 http.rs 机械外移（审查路线图第 4 周「拆上帝文件」样板段）：
//! 本模块零跨文件符号依赖，仅被 v1_scope 注册。

use actix_web::{get, post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ 附件/图床（0100，NP Pictured 最小落地） ============

const ATTACH_MAX_BYTES: usize = 8 * 1024 * 1024; // 单文件 8MiB
const ATTACH_MIME_ALLOW: [&str; 8] = [
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/avif",
    "application/pdf",
    "text/plain",
    // 0146 字幕链路：字幕包（zip/rar/7z）走附件存储；解包校验在字幕端白名单把关
    "application/zip",
];

/// 上传附件（multipart 字段 file）。存本地 savedirectory（缺省 ./attachments），
/// sha256 全站去重（同文件只存一份物理文件）；受 attach_quota_mib 配额约束。
/// 返回可直接在简介里引用的 /api/v1/attachments/{sha} URL。
#[post("/attachments")]
pub async fn upload_attachment(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    mut payload: actix_multipart::Multipart,
) -> DomainResult<HttpResponse> {
    use actix_web::web::Bytes;
    use futures_util::StreamExt;

    let auth = require_auth(&req, &state).await?;
    let mut file_bytes: Option<Bytes> = None;
    let mut filename = String::new();
    let mut mime = String::new();
    while let Some(item) = payload.next().await {
        let mut field =
            item.map_err(|e| DomainError::Validation(e.to_string()))?;
        if field.name() == Some("file") {
            filename = field
                .content_disposition()
                .and_then(|d| d.get_filename().map(str::to_string))
                .unwrap_or_default()
                .chars()
                .filter(|c| {
                    c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_'
                })
                .take(120)
                .collect();
            mime = field
                .content_type()
                .map(|m| m.essence_str().to_string())
                .unwrap_or_default();
            let mut buf = web::BytesMut::new();
            while let Some(chunk) = field.next().await {
                let chunk = chunk
                    .map_err(|e| DomainError::Validation(e.to_string()))?;
                buf.extend_from_slice(&chunk);
                if buf.len() > ATTACH_MAX_BYTES {
                    return Err(DomainError::Validation(
                        "附件超过 8MiB 上限".into(),
                    ));
                }
            }
            file_bytes = Some(buf.freeze());
        }
    }
    let Some(bytes) = file_bytes else {
        return Err(DomainError::Validation("缺少 file 字段".into()));
    };
    if !ATTACH_MIME_ALLOW.contains(&mime.as_str()) {
        return Err(DomainError::Validation(
            "仅支持 png/jpeg/gif/webp/avif/pdf/txt".into(),
        ));
    }
    // 真实内容嗅探（不信客户端头）：图片 magic bytes 校验；
    // .ass/.ssa 字幕在浏览器常被报为 text/plain——扩展名按文件名判、按文本嗅探放行；
    // zip 类字幕包按 PK 头嗅探（0146）
    let declared_txt = mime == "text/plain";
    let sniff_ok = match bytes.first() {
        Some(0x89) => {
            bytes.starts_with(&[0x89, b'P', b'N', b'G'])
                || mime == "application/pdf"
        }
        Some(0xFF) => mime == "image/jpeg",
        Some(b'G') => bytes.starts_with(b"GIF8"),
        Some(b'R') => bytes.starts_with(b"RIFF") && mime == "image/webp",
        Some(b'P') => bytes.starts_with(b"PK") && mime == "application/zip",
        _ => declared_txt || mime == "image/avif",
    };
    if !sniff_ok {
        return Err(DomainError::Validation("文件内容与声明类型不符".into()));
    }

    // 配额（attach_quota_mib，0=不限）：全站去重前先算已用
    let quota_mib: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'attach_quota_mib')::bigint, 512)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(512);
    if quota_mib > 0 {
        let used: i64 = sqlx::query_scalar(
            "SELECT COALESCE(sum(size), 0) FROM attachments WHERE user_id = $1",
        )
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
        if used + bytes.len() as i64 > quota_mib * 1024 * 1024 {
            return Err(DomainError::Validation(format!(
                "附件配额不足（已用 {}/{} MiB）",
                used / 1048576,
                quota_mib
            )));
        }
    }

    let sha = {
        use sha3::Digest;
        let mut h = sha3::Sha3_256::new();
        h.update(&bytes);
        let d = h.finalize();
        d.iter().map(|b| format!("{b:02x}")).collect::<String>()
    };
    // 去重：同 sha 已存在 → 直接复用（不重复占配额）
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM attachments WHERE sha256 = $1")
            .bind(&sha)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if exists.is_none() {
        // 0102 对象存储：按 storage_backend 分发（local 卷 / S3 兼容），目录结构与
        // 旧实现一致（sha 两级分片），读取端自动双后端回落——迁移期无缝。
        crate::storage::put(&state.repo.db, &sha, &bytes, &mime)
            .await
            .map_err(|e| DomainError::Internal(e))?;
        sqlx::query(
            "INSERT INTO attachments (user_id, sha256, filename, \
             mime, size) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(auth.id)
        .bind(&sha)
        .bind(if filename.is_empty() {
            format!("{sha}.{}", ext_of(&mime))
        } else {
            filename.clone()
        })
        .bind(&mime)
        .bind(bytes.len() as i64)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        state
            .repo
            .audit(Some(auth.id), "attachment_upload", None)
            .await;
    }
    Ok(ok(serde_json::json!({
        "sha256": sha,
        "url": format!("/api/v1/attachments/{sha}"),
        "size": bytes.len(),
        "deduplicated": exists.is_some(),
    })))
}

fn ext_of(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "application/pdf" => "pdf",
        _ => "txt",
    }
}

/// 附件读取（图床）：按 sha256 寻址（内容寻址不可猜测），mime 回放。
/// 需登录（防匿名爬图床占带宽）。
#[get("/attachments/{sha}")]
pub async fn get_attachment(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    require_auth(&req, &state).await?;
    let sha = path.into_inner();
    if sha.len() != 64 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(DomainError::Validation("sha256 格式无效".into()));
    }
    let row: Option<(String, i64)> =
        sqlx::query_as("SELECT mime, size FROM attachments WHERE sha256 = $1")
            .bind(&sha)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((mime, _size)) = row else {
        return Err(DomainError::NotFound(0));
    };
    let bytes = crate::storage::get(&state.repo.db, &sha)
        .await
        .ok_or(DomainError::NotFound(0))?;
    Ok(HttpResponse::Ok()
        .content_type(mime)
        .insert_header((
            actix_web::http::header::CACHE_CONTROL,
            "public, max-age=31536000, immutable",
        ))
        .body(bytes))
}
