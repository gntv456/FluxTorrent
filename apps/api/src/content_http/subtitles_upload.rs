//! 字幕上传链的私有辅助（300 行门禁：上传主链路与解析辅助分文件）。
//! 原来都堆在 subtitles.rs 里，文件被顶到 300 行门禁之上。

use actix_web::web;

use super::subtitles_util::{
    normalize_release, subtitle_ext_whitelist, subtitle_setting,
    SubtitleUploadReq,
};
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;
/// 文件三件套解析：attach://sha（体积/白名单/查重全链）或外部直链
pub(super) async fn resolve_file(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &crate::http::AuthUser,
    body: &SubtitleUploadReq,
    torrent_id: Option<i64>,
) -> DomainResult<(String, i64, Option<String>)> {
    let _ = torrent_id;
    if let Some(sha) =
        body.file_sha.as_deref().map(str::trim).filter(|s| {
            s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
        })
    {
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
            // 0285 P2：sha 字段是服务端算的 SHA3-256（字段名沿用历史契约），
            // 第三方工具用本地 SHA2 会落这里——文案必须指路正确的取值方式
            return Err(DomainError::Validation(
                "附件未上传或不存在（请先通过 POST /api/v1/attachments \
                 提交字幕文件，并原样回传其响应中的 sha256 字段值；\
                 该值是服务端按 SHA3-256 计算的，与本地 SHA2 不同）"
                    .into(),
            ));
        };
        // P0-2 体积上限（maxsubsize 接线；默认 1MB）
        let limit: i64 =
            subtitle_setting(&state.repo.db, "maxsubsize", "1048576")
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
        // 直链分支按人节流（throttle 5/min）：外链不改奖励口径，但防脚本批量
        // 提交外链刷字幕条数/占审核队列
        crate::http::throttle(state, format!("sub-link:{}", auth.id)).await?;
        Ok((link.to_string(), 0, None))
    } else {
        Err(DomainError::Validation(
            "请上传字幕文件（或提供 http/https 直链）".into(),
        ))
    }
}

/// release_name：优先客户端提供，缺省用种子名归一化
pub(super) async fn resolve_release(
    state: &web::Data<std::sync::Arc<AppState>>,
    torrent_id: Option<i64>,
    provided: Option<&str>,
) -> DomainResult<Option<String>> {
    if let Some(rn) = provided.map(str::trim).filter(|s| !s.is_empty()) {
        return Ok(Some(normalize_release(rn)));
    }
    match torrent_id {
        Some(tid) => sqlx::query_scalar::<_, String>(
            "SELECT name FROM torrents WHERE id = $1",
        )
        .bind(tid)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .map(|n| normalize_release(&n))
        .map_or(Ok(None), |v| Ok(Some(v))),
        None => Ok(None),
    }
}
