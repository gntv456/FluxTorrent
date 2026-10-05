//! 附件/图床（0100，NP Pictured 最小落地）。
//! 从 http.rs 机械外移（审查路线图第 4 周「拆上帝文件」样板段）：
//! 本模块零跨文件符号依赖，仅被 v1_scope 注册。

use actix_web::{get, post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ 附件/图床（0100，NP Pictured 最小落地） ============
// 单文件上限自 0214 起可配（attach_max_mib，缺省 8MiB），见 upload_attachment。

// 0285 P1：字幕链路全量打通——此前白名单/嗅探只放行 zip，注释却声称
// 「zip/rar/7z 走附件存储」，导致裸 .srt/.ass 与 .rar/.7z 包在附件层必被拒
// （前端 SUB_EXTS 与字幕端白名单都允许这些扩展名）。
const ATTACH_MIME_ALLOW: [&str; 14] = [
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/avif",
    "application/pdf",
    "text/plain",
    // 字幕文本（浏览器对 .ass/.ssa 常报 text/x-ssa 变体，统一收敛到这两个）
    "application/x-subrip",
    "application/x-ssa",
    // 字幕/未知扩展兜底：.sup/.idx/.sub/.cue 等浏览器一律报 octet-stream，
    // 内容层按「文本可解码或已知字幕/档案签名」二次校验（见 sniff_ok）
    "application/octet-stream",
    // 字幕包档案（0146 起设计意图，0285 补齐实现）
    "application/zip",
    "application/x-7z-compressed",
    "application/x-rar-compressed",
    "application/vnd.rar",
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
    // 单文件上限（0214 可配）：缺省 8MiB = 既有硬编码口径，0 = 不限
    let max_mib: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'attach_max_mib')::bigint, 8)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(8);
    let max_bytes = if max_mib <= 0 {
        usize::MAX
    } else {
        (max_mib as usize) * 1024 * 1024
    };
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
                if buf.len() > max_bytes {
                    return Err(DomainError::Validation(format!(
                        "附件超过 {max_mib}MiB 上限"
                    )));
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
            "仅支持 png/jpeg/gif/webp/avif/pdf/txt 与字幕文本 \
             （srt/ass）及字幕包（zip/rar/7z）"
                .into(),
        ));
    }
    // 真实内容嗅探（不信客户端头），按家族核对签名（0285 重写）：
    //   图片按 magic bytes；PDF 按 %PDF；zip 按 PK；rar 按 Rar!；7z 按 7z\xBC\xAF；
    //   字幕文本类（text/plain / x-subrip / x-ssa / octet-stream）按「UTF-8 可解码」；
    //   octet-stream 额外放行已知二进制字幕签名（PGS .sup 的 PG 头）。
    // 旧实现的缺陷：PDF（%PDF 开头）与裸字幕（octet-stream）落 default 分支被误杀。
    let sniff_ok = match mime.as_str() {
        "image/png" => bytes.starts_with(&[0x89, b'P', b'N', b'G']),
        "image/jpeg" => bytes.starts_with(&[0xFF, 0xD8]),
        "image/gif" => bytes.starts_with(b"GIF8"),
        "image/webp" => {
            bytes.len() > 12
                && bytes.starts_with(b"RIFF")
                && &bytes[8..12] == b"WEBP"
        }
        "image/avif" => bytes.len() > 12 && &bytes[4..8] == b"ftyp",
        "application/pdf" => bytes.starts_with(b"%PDF"),
        "application/zip" => bytes.starts_with(b"PK"),
        "application/x-7z-compressed" => {
            bytes.starts_with(b"7z\xBC\xAF\x27\x1C")
        }
        "application/x-rar-compressed" | "application/vnd.rar" => {
            bytes.starts_with(b"Rar!")
        }
        "application/x-subrip" | "application/x-ssa" | "text/plain" => {
            is_textlike(&bytes)
        }
        // octet-stream：文本可解码 → 字幕文本类；否则须命中已知二进制字幕签名
        "application/octet-stream" => {
            is_textlike(&bytes) || bytes.starts_with(b"PG")
        }
        _ => false,
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
        // sum(bigint)→numeric 必须显式 ::bigint，否则 i64 解码失败被
        // unwrap_or 吞掉 → 配额已用量恒 0（等于配额永不生效）
        let used: i64 = sqlx::query_scalar(
            "SELECT COALESCE(sum(size), 0)::bigint FROM attachments WHERE user_id = $1",
        )
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(%e, "附件配额已用量查询失败");
            0
        });
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
        // 0285 P2：字段名沿用 sha256（历史契约），算法实为 SHA3-256——
        // 显式标注供第三方工具正确计算；本地算的 SHA2 会校验失败
        "algorithm": "sha3-256",
        "url": format!("/api/v1/attachments/{sha}"),
        "size": bytes.len(),
        "deduplicated": exists.is_some(),
    })))
}

/// 文本类嗅探：UTF-8 可解码且不含 NUL 即视为字幕文本（srt/ass/ssa/cue/idx
/// 均为纯文本；只查前 8KiB 防 8MiB 大文件全量扫描）。
fn is_textlike(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(8192)];
    std::str::from_utf8(head).is_ok() && !head.contains(&0u8)
}

fn ext_of(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "application/pdf" => "pdf",
        "application/x-subrip" => "srt",
        "application/x-ssa" => "ass",
        "application/zip" => "zip",
        "application/x-7z-compressed" => "7z",
        "application/x-rar-compressed" | "application/vnd.rar" => "rar",
        "application/octet-stream" => "bin",
        _ => "txt",
    }
}

/// 附件读取（图床）：按 sha256 寻址（内容寻址不可猜测），mime 回放。
/// 需登录（防匿名爬图床占带宽）。Range/206 支持在 attachment_video.rs（0190）。
#[get("/attachments/{sha}")]
pub async fn get_attachment(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    crate::attachment_video::serve_attachment(req, state, path).await
}
