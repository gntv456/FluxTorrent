//! 发种入库中段（M04）：文件清单 files / 自动促销（0089）/ 推荐位（0089）。
//! 从 publish_http/upload.rs 按域拆出。

use actix_web::web;

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::ptgen::UploadForm;

#[allow(clippy::too_many_arguments)]
pub(super) async fn store_files_promo(
    state: &web::Data<std::sync::Arc<AppState>>,
    form: &UploadForm,
    parsed: &crate::bencode::ParsedTorrent,
    id: i64,
    auth: &crate::http::AuthUser,
) -> DomainResult<()> {
    // 文件清单入 files 表（修复前从不写入：新种的文件列表/按文件名搜索永远为空）
    for (idx, (path, len)) in parsed.files.iter().enumerate() {
        sqlx::query(
            "INSERT INTO files (torrent_id, file_index, \
                 path, size) VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind(idx as i32)
        .bind(path)
        .bind(len)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }

    // 发种自动促销（0089，NP 促销设置 口径）：管理后台配置默认促销（类型+天数），
    // 发布即自动套用——促销跟随站点，不再由发布者单独设置。
    let auto_kind: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
             WHERE name = 'upload_auto_promo_kind'), '')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_default();
    let auto_days: i32 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::int FROM site_settings \
             WHERE name = 'upload_auto_promo_days'), 0)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    let auto_kind = auto_kind.trim().to_lowercase();
    if !auto_kind.is_empty()
        && auto_days > 0
        && ["free", "x2", "x2free", "half", "x2half", "p30"]
            .contains(&auto_kind.as_str())
    {
        sqlx::query(
                "INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source, created_by) \
                 VALUES ('torrent', $1, $2::promotion_kind_enum, now(), now() + make_interval(days => $3), \
                         'manual'::promotion_source, $4)",
            )
            .bind(id)
            .bind(&auto_kind)
            .bind(auto_days.clamp(1, 720))
            .bind(auth.id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }

    // 推荐位（0089，NP 挑选 口径）：置顶位置/截止 + 推荐影片，管理组专属；
    // 发布页人人可见，无权限提交会被此处拦截（与参考站服务端强校验同口径）
    if form.pos_state.unwrap_or(0) != 0
        || form.pick_type.unwrap_or(0) != 0
        || form
            .pos_state_until
            .as_deref()
            .map(str::trim)
            .is_some_and(|s| !s.is_empty())
    {
        if auth.class_id < 90 {
            return Err(DomainError::Forbidden); // 置顶/推荐仅管理组
        }
        let pos = form.pos_state.unwrap_or(0);
        if ![0, 1, 2].contains(&pos) {
            return Err(DomainError::Validation("置顶位置取值 0/1/2".into()));
        }
        let pick = form.pick_type.unwrap_or(0);
        if ![0, 1, 2].contains(&pick) {
            return Err(DomainError::Validation("推荐影片取值 0/1/2".into()));
        }
        let until = form
            .pos_state_until
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|_| {
                        DomainError::Validation("置顶截止时间格式无效".into())
                    })
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            })
            .transpose()?;
        sqlx::query(
            "UPDATE torrents SET pos_state = $2, \
                 pos_state_until = $3, pick_type = $4, \
                 mtime = now() WHERE id = $1",
        )
        .bind(id)
        .bind(pos)
        .bind(until)
        .bind(pick)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }

    state
        .repo
        .audit(Some(auth.id), "torrent_upload", Some(id))
        .await;
    Ok(())
}
