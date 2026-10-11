//! 字幕下载与语言字典（0146/0148）：从 subtitles_list.rs 按域拆出
//! （300 行门禁；列表/筛选/AI 三态在 subtitles_list.rs）。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 字幕下载：计数 +1；本地附件直接回文件字节。0146：扩展名按 mime + 落库 ext
/// 决定（字幕格式直通，兜底 .srt 而非 .bin），文件名带语言代码。
/// 挂靠种子的字幕跟种子审批走（待审/被拒/回收 ⇒ 仅 see_banned 可取）。
#[get("/subtitles/{id}/download")]
pub(super) async fn subtitle_download(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let sid = path.into_inner();
    let can_see_banned = crate::authz::can(
        &state,
        &auth,
        crate::authz::perm::TORRENT_SEE_BANNED,
    )
    .await;
    // (file_ref, torrent_id, title, lang, ext)
    type DlRow = (String, Option<i64>, String, Option<String>, Option<String>);
    let row: Option<DlRow> = sqlx::query_as(
        "SELECT s.file_ref, s.torrent_id, s.title, s.lang, s.ext \
         FROM subtitles s \
         LEFT JOIN torrents t ON t.id = s.torrent_id \
         WHERE s.id = $1 AND s.deleted_at IS NULL AND s.status = 1 \
           AND (t.id IS NULL OR t.approval_status = 1 OR $2::bool)",
    )
    .bind(sid)
    .bind(can_see_banned)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((file_ref, torrent_id, title, lang, sub_ext)) = row else {
        return Err(DomainError::NotFound(sid));
    };
    // C2：下载计数——见 bump_download（P-48 起只在成功出口计）
    // C1：附件可见性闸门——字幕绕过附件接口直取他人 private 附件字节
    // （借道 subtitles/{id}/download 不走 attachment_video 的 visibility 检查）。
    if let Some(sha) = file_ref.strip_prefix("attach://") {
        if let Some(true) = crate::attachment_video::visibility_denied(
            &state.repo.db,
            auth.id,
            sha,
        )
        .await?
        {
            return Err(DomainError::Forbidden);
        }
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
        // P-48：计一次——只有真的取到字节才算一次下载
        bump_download(&state, sid, auth.id).await?;
        let safe_title = safe_filename(&title);
        // 扩展名优先级：落库 ext（上传时按文件名判定）> mime 常见映射 > .srt
        let ext =
            sub_ext.filter(|e| !e.is_empty()).unwrap_or_else(|| {
                match mime.as_str() {
                    "application/pdf" => "pdf".into(),
                    _ => "srt".into(),
                }
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
    // 外链字幕：把 URL 交给前端新窗口打开，这一跳就是一次下载（P-48 起
    // 也只在成功出口计）
    bump_download(&state, sid, auth.id).await?;
    Ok(ok(serde_json::json!({
        "id": sid,
        "title": title,
        "torrent_id": torrent_id,
        "file_ref": file_ref,
    })))
}

/// 计一次下载（C2 去重：同人同字幕每小时一次；Redis 故障按首次计，
/// 统计面不 fail-close 拦下载）。
/// P-48（本批实测）：调用点必须在**成功出口**——原实现把计数放在函数
/// 开头，于是可见性被拒（403）与取不到字节（404）也照样 +1（实测 404
/// 一次 downloads 0→1）。这个数字进月度评选加权与列表排序，等于用坏链
/// 或越权探测就能刷排名。
async fn bump_download(
    state: &web::Data<std::sync::Arc<AppState>>,
    sid: i64,
    uid: i64,
) -> DomainResult<()> {
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let k = format!("subdl:{sid}:{uid}");
    // SET k 1 NX EX（与 twofa 重放闸同式）：nil=本小时内已计过
    let fresh: Option<()> = redis::cmd("SET")
        .arg(&k)
        .arg(1i64)
        .arg("NX")
        .arg("EX")
        .arg(3600u64)
        .query_async(&mut c)
        .await
        .unwrap_or(None);
    if fresh.is_none() {
        return Ok(());
    }
    sqlx::query("UPDATE subtitles SET downloads = downloads + 1 WHERE id = $1")
        .bind(sid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
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
