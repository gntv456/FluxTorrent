//! 发种入库中段（M04）：文件清单 files / 自动促销（0089）/ 推荐位（0089）。
//! 从 publish_http/upload.rs 按域拆出。

use actix_web::web;

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

#[allow(clippy::too_many_arguments)]
pub(super) async fn store_files_promo(
    state: &web::Data<std::sync::Arc<AppState>>,
    parsed: &crate::bencode::ParsedTorrent,
    id: i64,
    auth: &crate::http::AuthUser,
    promo: Option<super::upload_precheck::PromoSet>,
) -> DomainResult<()> {
    // 文件清单入 files 表（修复前从不写入：新种的文件列表/按文件名搜索永远为空）
    // 0288：改多值批量 INSERT —— 逐行 await 的写法在实测 5000 文件种子上是 5000 次往返。
    if !parsed.files.is_empty() {
        let mut qb = sqlx::QueryBuilder::new(
            "INSERT INTO files (torrent_id, file_index, path, size) ",
        );
        qb.push_values(
            parsed.files.iter().enumerate(),
            |mut b, (idx, (path, len))| {
                b.push_bind(id)
                    .push_bind(idx as i32)
                    .push_bind(path)
                    .push_bind(*len);
            },
        );
        qb.push(" ON CONFLICT DO NOTHING")
            .build()
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

    // 推荐位（0089，NP 挑选 口径）：置顶位置/截止 + 推荐影片，管理组专属。
    // 0288：权限/取值/时间格式的校验已全部前置到 `upload_precheck::check_promo`
    // （建种子行之前），这里只写已判定合法的值——过去这段跑在 INSERT 之后，
    // 越权提交会留下一枚待审残种且同一 .torrent 永久判重。
    if let Some(p) = promo {
        sqlx::query(
            "UPDATE torrents SET pos_state = $2, \
                 pos_state_until = $3, pick_type = $4, \
                 mtime = now() WHERE id = $1",
        )
        .bind(id)
        .bind(p.pos)
        .bind(p.until)
        .bind(p.pick)
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
