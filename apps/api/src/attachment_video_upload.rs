//! 论坛视频内嵌 V2（0190）——上传子域（从 attachment_video.rs 拆出，300 门禁）：
//! multipart 解析 / 魔数校验 / 独立配额与每日频率 / sha256 落库。
//! 读取端（Range/206/HEAD）仍在 attachment_video.rs。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::http::AuthUser;
use crate::state::AppState;

pub(super) fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

/// mp4：[0..4] 任意 box size，[4..8] = 'ftyp'
pub(super) fn is_mp4(b: &[u8]) -> bool {
    b.len() >= 12 && &b[4..8] == b"ftyp"
}

/// webm/mkv：EBML 魔数 0x1A45DFA3
pub(super) fn is_webm(b: &[u8]) -> bool {
    b.starts_with(&[0x1A, 0x45, 0xDF, 0xA3])
}

pub(super) async fn setting_i64(
    db: &sqlx::PgPool,
    name: &str,
    default: i64,
) -> i64 {
    // 开关语义：值是 'yes'/'no' 或数字。先把 yes/no 翻成 1/0 再 ::bigint，
    // 数字键（上限/配额）直接转换；任何非法值回落缺省（fail-close 语义
    // 由调用方决定：开关默认 0=关，上限默认具体值）。
    sqlx::query_scalar(&format!(
        "SELECT COALESCE((SELECT CASE WHEN LOWER(value) = 'yes' THEN '1' \
         WHEN LOWER(value) = 'no' THEN '0' ELSE value END FROM site_settings \
         WHERE name = '{name}'), '{default}')::bigint"
    ))
    .fetch_one(db)
    .await
    .unwrap_or(default)
}

#[derive(Deserialize)]
pub(super) struct VideoMeta {
    pub duration: Option<f64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub poster_sha: Option<String>,
}

/// 上传站内视频（multipart：file + 可选 meta JSON 字段）。
/// 总开关 forum_video_upload=no → 403；成功返回 !video() 可用的站内 URL。
/// 分账：视频只吃 video_quota_mib / video_daily_limit（0190），不吃图片配额。
#[post("/attachments/video")]
pub async fn upload_video(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    mut payload: actix_multipart::Multipart,
) -> DomainResult<HttpResponse> {
    use actix_web::web::Bytes;
    use futures_util::StreamExt;

    let auth = require_auth(&req, &state).await?;
    let on = setting_i64(&state.repo.db, "forum_video_upload", 0).await;
    if on == 0 {
        return Err(DomainError::Forbidden);
    }
    let max_mib = setting_i64(&state.repo.db, "video_max_mib", 200).await;
    if max_mib <= 0 {
        return Err(DomainError::Validation("视频上传已停用".into()));
    }
    let max_bytes = (max_mib as usize) * 1024 * 1024;

    let mut file_bytes: Option<Bytes> = None;
    let mut filename = String::new();
    let mut meta_raw = String::new();
    while let Some(item) = payload.next().await {
        let mut field =
            item.map_err(|e| DomainError::Validation(e.to_string()))?;
        match field.name() {
            Some("file") => {
                filename = field
                    .content_disposition()
                    .and_then(|d| d.get_filename().map(str::to_string))
                    .unwrap_or_default()
                    .chars()
                    .filter(|c| {
                        c.is_alphanumeric()
                            || *c == '.'
                            || *c == '-'
                            || *c == '_'
                    })
                    .take(120)
                    .collect();
                let mut buf = web::BytesMut::new();
                while let Some(chunk) = field.next().await {
                    let chunk = chunk
                        .map_err(|e| DomainError::Validation(e.to_string()))?;
                    buf.extend_from_slice(&chunk);
                    if buf.len() > max_bytes {
                        return Err(DomainError::Validation(format!(
                            "视频超过 {max_mib}MiB 上限"
                        )));
                    }
                }
                file_bytes = Some(buf.freeze());
            }
            Some("meta") => {
                let mut buf = web::BytesMut::new();
                while let Some(chunk) = field.next().await {
                    let chunk = chunk
                        .map_err(|e| DomainError::Validation(e.to_string()))?;
                    buf.extend_from_slice(&chunk);
                    if buf.len() > 4096 {
                        break;
                    }
                }
                meta_raw = String::from_utf8_lossy(&buf).to_string();
            }
            _ => {}
        }
    }
    let Some(bytes) = file_bytes else {
        return Err(DomainError::Validation("缺少 file 字段".into()));
    };

    // 魔数（声明 mime/扩展名不可信）：mp4 或 webm 二选一
    let mime = if is_mp4(&bytes) {
        "video/mp4"
    } else if is_webm(&bytes) {
        "video/webm"
    } else {
        return Err(DomainError::Validation(
            "视频内容校验失败（仅支持 mp4/webm）".into(),
        ));
    };
    check_quota_and_rate(&state, auth.id, bytes.len()).await?;

    let meta: VideoMeta = if meta_raw.is_empty() {
        VideoMeta {
            duration: None,
            width: None,
            height: None,
            poster_sha: None,
        }
    } else {
        serde_json::from_str(&meta_raw).map_err(|_| {
            DomainError::Validation("meta 字段格式无效".into())
        })?
    };
    persist(&state, &auth, &bytes, mime, &filename, &meta).await
}

/// 配额分账（video_quota_mib）+ 每日频率（video_daily_limit，0=不限）。
async fn check_quota_and_rate(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
    len: usize,
) -> DomainResult<()> {
    let quota_mib =
        setting_i64(&state.repo.db, "video_quota_mib", 2048).await;
    if quota_mib > 0 {
        let used: i64 = sqlx::query_scalar(
            "SELECT COALESCE(sum(size), 0) FROM attachments \
             WHERE user_id = $1 AND kind = 'video'",
        )
        .bind(uid)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
        if used + len as i64 > quota_mib * 1024 * 1024 {
            return Err(DomainError::Validation(format!(
                "视频配额不足（已用 {}/{} MiB）",
                used / 1048576,
                quota_mib
            )));
        }
    }
    let daily = setting_i64(&state.repo.db, "video_daily_limit", 5).await;
    if daily > 0 {
        let today: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM attachments \
             WHERE user_id = $1 AND kind = 'video' \
             AND created_at::date = current_date",
        )
        .bind(uid)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
        if today >= daily {
            return Err(DomainError::Validation(format!(
                "今日视频上传已达上限（{daily} 条/天）"
            )));
        }
    }
    Ok(())
}

/// sha256 全站去重落库（重复文件复用，不重复占配额）。
async fn persist(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &AuthUser,
    bytes: &[u8],
    mime: &str,
    filename: &str,
    meta: &VideoMeta,
) -> DomainResult<HttpResponse> {
    let sha = {
        use sha3::Digest;
        let mut h = sha3::Sha3_256::new();
        h.update(bytes);
        let d = h.finalize();
        d.iter().map(|b| format!("{b:02x}")).collect::<String>()
    };
    let exists: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM attachments WHERE sha256 = $1",
    )
    .bind(&sha)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(internal)?;
    if exists.is_none() {
        crate::storage::put(&state.repo.db, &sha, bytes, mime)
            .await
            .map_err(|e| DomainError::Internal(e))?;
        let name = if filename.is_empty() {
            let ext = if mime == "video/mp4" { "mp4" } else { "webm" };
            format!("{sha}.{ext}")
        } else {
            filename.to_string()
        };
        sqlx::query(
            "INSERT INTO attachments (user_id, sha256, filename, mime, \
             size, kind, metadata) VALUES ($1,$2,$3,$4,$5,'video',$6)",
        )
        .bind(auth.id)
        .bind(&sha)
        .bind(name)
        .bind(mime)
        .bind(bytes.len() as i64)
        .bind(serde_json::json!({
            "duration": meta.duration,
            "width": meta.width, "height": meta.height,
            "poster_sha": meta.poster_sha,
        }))
        .execute(&state.repo.db)
        .await
        .map_err(internal)?;
        state
            .repo
            .audit(Some(auth.id), "video_upload", None)
            .await;
    }
    Ok(ok(serde_json::json!({
        "sha256": sha,
        "url": format!("/api/v1/attachments/{sha}"),
        "mime": mime,
        "size": bytes.len(),
        "deduplicated": exists.is_some(),
    })))
}
