//! 详情页「辅种」行（可选插件，默认关）。
//!
//! 只回**本站种子 → 别站也有**这个方向的数据；开关、密钥、接口地址
//! 全在 `iyuu_bridge` 里，本文件只负责鉴权 + 取指纹 + 拼响应。
//!
//! 关闭时零出站：`enabled=false` 直接返回，连 IYUU 的域名都不碰。

use actix_web::{get, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::iyuu_bridge::{self, Hit};
use crate::state::AppState;

/// 详情页辅种行响应。
#[derive(serde::Serialize)]
struct CrossseedOut {
    /// 站长是否启用了本插件
    enabled: bool,
    /// 是否已配置认证密钥（enabled 但没密钥时前端给站长提示）
    configured: bool,
    /// 命中的别站种子
    sites: Vec<Hit>,
}

#[utoipa::path(
    get,
    path = "/api/v1/torrents/{id}/crossseed",
    tag = "种子",
    description = "详情页辅种行：本站种子在别的站的同种（可选插件）。\
            未启用时返回 enabled=false 且不发任何出站请求；\
            已启用但 IYUU 不可达时返回空列表（不报错）。",
    params(("id" = i64, description = "种子 id")),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
    )
)]
#[get("/torrents/{id}/crossseed")]
pub(crate) async fn torrent_crossseed(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let conf = iyuu_bridge::conf(&state).await;
    if !conf.enabled {
        return Ok(ok(CrossseedOut {
            enabled: false,
            configured: conf.ready(),
            sites: Vec::new(),
        }));
    }

    // 种子必须存在且可见，否则按 404 语义返回（面板不显示）。
    // 这里只校验存在性——指纹不参与，查重早由 worker 算好存表了。
    let exists: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM torrents WHERE id = $1 AND approval_status = 1",
    )
    .bind(id)
    .fetch_optional(&state.repo.read_db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if exists.is_none() {
        return Err(DomainError::NotFound(id));
    }

    // 零出站：只读 iyuu_crossseed（worker 每小时批量刷）。
    let sites = iyuu_bridge::hits_of(&state, &conf, id).await;
    Ok(ok(CrossseedOut {
        enabled: true,
        configured: conf.ready(),
        sites,
    }))
}
