//! 开放 API：种子深详情（0268）。
//!
//! 兼容层的 `torrent/{id}.json` 只有 12 个字段，做「详情页」的工具
//! （移动壳 / 聚合搜索预览）拿不到文件清单、MediaInfo、多维属性。
//! 本端点把站内详情页的**同一批数据**按开放口径输出。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;
use crate::torrents::{get_torrent, get_torrent_detail, list_files, CatMap};

use super::{no_store, require_scope, require_token, with_rl, SCOPE_READ};

/// GET /open/torrents/{id}?with_nfo=1 —— 种子深详情。
///
/// 只放行**过审**种（与站内浏览页同口径）；`with_nfo=1` 才带 NFO 全文
/// （默认不带：NFO 动辄几十 KB，列表预览场景没必要）。
#[get("/open/torrents/{id}")]
pub(super) async fn open_torrent_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    require_scope(&tk, SCOPE_READ)?;
    let id = path.into_inner();
    // viewer=None：只看过审种，不做 owner/staff 放宽（对外口径从严）
    let row = get_torrent(&state.repo.db, id, false, None).await?;
    let detail = get_torrent_detail(&state.repo.db, id, tk.uid).await?;
    let files = list_files(&state.repo.db, id).await?;
    let cats = CatMap::load(&state.repo.db).await;
    let with_nfo = matches!(
        q.get("with_nfo").map(String::as_str),
        Some("1") | Some("true")
    );
    let nfo: Option<String> = if with_nfo {
        sqlx::query_scalar("SELECT nfo FROM torrents WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten()
    } else {
        None
    };
    let (dlf, ulf) =
        crate::torrents::promo::factors(row.promotion.as_deref().unwrap_or(""));
    // 分两段构建再合并：单条 json! 太大撑爆递归展开上限
    let mut body = serde_json::json!({
        "id": row.id,
        "name": row.name,
        "small_descr": row.small_descr,
        "descr": detail.descr,
        "size": row.size,
        "numfiles": detail.numfiles,
        "seeders": row.seeders,
        "leechers": row.leechers,
        "completed": row.times_completed,
        "added": row.created_at.timestamp(),
        "category": row.category_id,
        "category_name": cats.name(row.category_id),
        "category_np": cats.legacy(row.category_id),
        "category_newznab": cats.newznab(row.category_id),
        "medium_id": row.medium_id,
        "imdb_id": row.imdb_id,
        "rating": row.rating,
        "poster": row.poster,
        "official": row.official_tag,
        "anonymous": row.anonymous,
        "owner_name": row.owner_name,
        "sticky": row.sticky,
    });
    let extra = serde_json::json!({
        "info_hash": row.info_hash,
        "pieces_hash": row.pieces_hash,
        "promotion": row.promotion,
        "promotion_ends_at": row.promotion_ends_at,
        "downloadvolumefactor": dlf,
        "uploadvolumefactor": ulf,
        "hr_policy": row.hr_policy,
        "price": detail.price,
        "thanks_count": detail.thanks_count,
        "bookmark_count": detail.bookmark_count,
        "last_action": detail.last_action,
        "sections": detail.sections,
        "mediainfo": detail.mediainfo,
        "nfo": nfo,
        "download": format!(
            "/api/v1/compat/nexusphp/download.php?id={id}&passkey=<passkey>"
        ),
        "files": files
            .into_iter()
            .map(|f| {
                serde_json::json!({
                    "index": f.file_index, "path": f.path, "size": f.size,
                })
            })
            .collect::<Vec<_>>(),
    });
    if let (Some(a), Some(b)) = (body.as_object_mut(), extra.as_object()) {
        a.extend(b.clone());
    }
    Ok(no_store(with_rl(ok(body), &tk)))
}
