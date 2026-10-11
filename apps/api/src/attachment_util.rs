//! 附件体系的共用小工具（300 行门禁按域拆分，批二十九）。
//! 上传入口在 attachment_http.rs / attachment_video_upload.rs，
//! 读取入口（Range/HEAD/可见性）在 attachment_video.rs。

use sqlx::PgPool;

/// 附件功能总开关（0034 起就在界面上）。缺省按「开」处理：这一项读不到
/// 或读失败时维持既有行为，与站点其它 setting 缺省口径一致。
pub(crate) async fn attach_enabled(db: &sqlx::PgPool) -> bool {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'enableattach'), 'yes') <> 'no'",
    )
    .fetch_one(db)
    .await
    .unwrap_or(true)
}

pub(crate) fn is_textlike(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(8192)];
    std::str::from_utf8(head).is_ok() && !head.contains(&0u8)
}

pub(crate) fn ext_of(mime: &str) -> &'static str {
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

