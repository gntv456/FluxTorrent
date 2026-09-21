//! M17 字幕区核心链路（上传/列表/下载 + 0146 治理加固）。
//! 元数据 CRUD、评分举报、语言字典在 subtitles_meta.rs；共用辅助/请求体在
//! subtitles_util.rs（300 行门禁按域拆分）。

use actix_web::{post, web, HttpRequest, HttpResponse};

use super::subtitles_util::{
    normalize_release, subtitle_ext_whitelist, subtitle_setting,
    SubtitleUploadMeta, SubtitleUploadReq,
};
use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 上传（0146 加固）：体积上限 + 扩展名白名单（按 kind）+ sha 查重防刷火花 +
/// 元数据落库 + 审核态判定。历史直链（http/https）不受体积/白名单校验。
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
    // 审核开关（0146）：开 → status=0 待审；关 → 发布即生效（NP 口径）
    let moderation =
        subtitle_setting(&state.repo.db, "subtitle_moderation", "0").await?
            == "1";
    let (file_ref, size, ext) = resolve_file(
        &state, &auth, &body, torrent_id,
    )
    .await?;
    let meta = SubtitleUploadMeta::validate(&body)?;
    let release_name = resolve_release(
        &state, torrent_id, meta.release_name.as_deref(),
    )
    .await?;
    // lang → lang_id（0146 字典表；旧 lang 字符串同步写，读端双轨过渡）
    let lang_id: Option<i16> = match body.lang.as_deref().map(str::trim) {
        Some(code) if !code.is_empty() && code != "0" => {
            sqlx::query_scalar::<_, i16>(
                "SELECT id FROM subtitle_langs WHERE code = $1",
            )
            .bind(code)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
        }
        _ => None,
    };
    // 0148 C1：挂了种子 → 回填种子 imdb_id（同片合并的行级冗余）
    let imdb_id: Option<String> = match torrent_id {
        Some(tid) => sqlx::query_scalar(
            "SELECT imdb_id FROM torrents WHERE id = $1",
        )
        .bind(tid)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .flatten(),
        None => None,
    };
    let status: i16 = if moderation { 0 } else { 1 };
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO subtitles (torrent_id, user_id, title, lang, lang_id, \
         file_ref, size, ext, fps, machine_translated, hearing_impaired, \
         foreign_parts_only, source, producer, proofreader, author_name, \
         release_name, anon, status, imdb_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, \
         $14, $15, $16, $17, $18, $19, $20) RETURNING id",
    )
    .bind(torrent_id)
    .bind(auth.id)
    .bind(body.title.trim())
    .bind(&body.lang)
    .bind(lang_id)
    .bind(&file_ref)
    .bind(size)
    .bind(&ext)
    .bind(meta.fps)
    .bind(meta.machine_translated)
    .bind(meta.hearing_impaired)
    .bind(meta.foreign_parts_only)
    .bind(&meta.source)
    .bind(&meta.producer)
    .bind(&meta.proofreader)
    .bind(&meta.author_name)
    .bind(release_name)
    .bind(meta.anon)
    .bind(status)
    .bind(imdb_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 发字幕 +5 火花（旧站口径）
    let idem = format!("subtitle:{}:{}", auth.id, id);
    earn_spark(&state.repo.db, auth.id, 5, "subtitle", &idem).await?;
    announce_upload(&state, &auth, &meta, body.title.trim()).await;
    Ok(ok(serde_json::json!({ "id": id, "reward": 5, "status": status })))
}

/// 文件三件套解析：attach://sha（体积/白名单/查重全链）或外部直链
async fn resolve_file(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &crate::http::AuthUser,
    body: &SubtitleUploadReq,
    torrent_id: Option<i64>,
) -> DomainResult<(String, i64, Option<String>)> {
    let _ = torrent_id;
    if let Some(sha) = body.file_sha.as_deref().map(str::trim).filter(|s| {
        s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
    }) {
        // 校验附件归属：必须是本人在 attachments 上传过的文件（防冒用他人 sha）
        let owned: Option<(i64, String)> = sqlx::query_as(
            "SELECT size, filename FROM attachments WHERE sha256 = $1 \
             AND user_id = $2",
        )
        .bind(sha)
        .bind(auth.id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let Some((fsize, fname)) = owned else {
            return Err(DomainError::Validation(
                "附件未上传或不存在（请先通过上传接口提交字幕文件）".into(),
            ));
        };
        // P0-2 体积上限（maxsubsize 接线；默认 1MB）
        let limit: i64 = subtitle_setting(
            &state.repo.db,
            "maxsubsize",
            "1048576",
        )
        .await?
        .parse()
        .unwrap_or(1048576);
        if fsize > limit {
            return Err(DomainError::Validation(format!(
                "字幕体积超限（{fsize} > {limit} 字节）"
            )));
        }
        // P0-3 扩展名白名单：按 kind 取（lyric 允许 lrc，影视禁 lrc）
        let file_ext = fname
            .rsplit('.')
            .next()
            .map(|e| e.to_ascii_lowercase())
            .filter(|_| fname.contains('.'));
        let whitelist = subtitle_ext_whitelist(&state.repo.db).await?;
        if let Some(e) = file_ext.as_ref() {
            if !whitelist.contains(e) {
                return Err(DomainError::Validation(format!(
                    "不允许的扩展名 .{e}（允许：{}）",
                    whitelist.join("/")
                )));
            }
        }
        // P0-4 sha 查重：同文件已挂在未删字幕下 → 拒绝（防重复 +5 火花）
        let dup: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM subtitles WHERE file_ref = $1 \
             AND deleted_at IS NULL)",
        )
        .bind(format!("attach://{sha}"))
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        if dup {
            return Err(DomainError::Validation(
                "该字幕已存在（相同文件）".into(),
            ));
        }
        Ok((format!("attach://{sha}"), fsize, file_ext))
    } else if let Some(link) = body
        .file_ref
        .as_deref()
        .map(str::trim)
        .filter(|s| s.starts_with("http://") || s.starts_with("https://"))
    {
        Ok((link.to_string(), 0, None))
    } else {
        Err(DomainError::Validation(
            "请上传字幕文件（或提供 http/https 直链）".into(),
        ))
    }
}

/// release_name：优先客户端提供，缺省用种子名归一化
async fn resolve_release(
    state: &web::Data<std::sync::Arc<AppState>>,
    torrent_id: Option<i64>,
    provided: Option<&str>,
) -> DomainResult<Option<String>> {
    if let Some(rn) = provided.map(str::trim).filter(|s| !s.is_empty()) {
        return Ok(Some(normalize_release(rn)));
    }
    match torrent_id {
        Some(tid) => {
            sqlx::query_scalar::<_, String>(
                "SELECT name FROM torrents WHERE id = $1",
            )
            .bind(tid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .map(|n| normalize_release(&n))
            .map_or(Ok(None), |v| Ok(Some(v)))
        }
        None => Ok(None),
    }
}

/// 公告联动（P2-3）：shoutbox 系统消息（匿名则「一位匿名用户」）。
/// 失败静默——公告不可用不应阻断上传主链路
async fn announce_upload(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &crate::http::AuthUser,
    meta: &SubtitleUploadMeta,
    title: &str,
) {
    let display = if meta.anon {
        "一位匿名用户".to_string()
    } else if let Some(name) = &meta.author_name {
        name.clone()
    } else {
        sqlx::query_scalar::<_, String>(
            "SELECT username FROM users WHERE id = $1",
        )
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or_else(|_| "用户".into())
    };
    let _ = sqlx::query(
        "INSERT INTO shoutbox (user_id, message) VALUES ($1, $2)",
    )
    .bind(auth.id)
    .bind(format!("{display} 上传了字幕「{title}」"))
    .execute(&state.repo.db)
    .await;
}
