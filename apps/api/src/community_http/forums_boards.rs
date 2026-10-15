//! 版块看板口径（GET /forums/boards）从 forums.rs 按域拆出。
//! forums.rs 加 utoipa 注解后贴近 300 行门禁，按域拆出。
//!
//! 2026-10-16：SQL 已下沉到 repo::community::boards（H 层不 import
//! sqlx，FRAME §2）。

use actix_web::{get, web, HttpRequest, HttpResponse};
use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::repo::community::boards as repo_boards;
use crate::state::AppState;

pub(crate) use crate::repo::community::boards::BoardBrief;

/// 全站版块精简列表（「移动主题到…」下拉的数据源）。
/// 口径与 forum_list 的版块过滤一致：只回当前用户可读的版块，不泄漏隐藏版块名。
#[utoipa::path(
    get,
    path = "/api/v1/forums/boards",
    tag = "社区/论坛",
    description = "版块看板口径（活跃度排序的简化列表，供首页用）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/forums/boards")]
pub(crate) async fn forum_boards(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let is_staff = auth.class_id >= 93;
    let rows = repo_boards::list_boards(
        &state.repo,
        auth.id,
        auth.class_id,
        is_staff,
    )
    .await?;
    Ok(ok(rows))
}
