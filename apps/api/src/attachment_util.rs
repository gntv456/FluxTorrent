//! 附件体系的共用小工具（300 行门禁按域拆分，批二十九）。
//! 上传入口在 attachment_http.rs / attachment_video_upload.rs，
//! 读取入口（Range/HEAD/可见性）在 attachment_video.rs。

use sqlx::PgPool;

/// 附件查看最低等级（后台「附件查看最低等级」，0=不限）。
///
/// P-64（批二十九）：这条 `settings_meta` 登记项此前零消费——站长设成 1
/// 之后任何注册用户照样读得到所有 shared 附件，界面上却写着
/// 「低于此等级不可查看附件」。读取的三个出口（GET /attachments/{sha}、
/// HEAD、字幕 attach:// 直取）共用这里；staff（class≥90）不受限，与站点
/// 其它等级门槛同口径。取不到/解析失败按「不限」处理并记 warn：
/// 这一项是**附加限制**，读失败时把全站图片锁死比放过限制更糟。
pub(crate) async fn attach_class_ok(db: &PgPool, class_id: i32) -> bool {
    if class_id >= 90 {
        return true;
    }
    let min: i64 = match sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'attachment_authority'), '0')::bigint",
    )
    .fetch_one(db)
    .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(%e, "attachment_authority 读取失败，按不限处理");
            return true;
        }
    };
    min <= 0 || class_id as i64 >= min
}

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

