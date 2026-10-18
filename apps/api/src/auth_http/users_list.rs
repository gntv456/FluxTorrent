//! 会员列表（GET /users）——「人」的公开目录，登录后可读。
//!
//! SQL 全部住在 repo/community/user_directory.rs（H 层零容忍带 SQL）。
//!
//! 口径（PT 站惯例）：
//! * 未登录不进目录（`require_auth`），但**不需要任何权限键**——它是
//!   成员面，不是职员面；`/admin/users` 那套 `USER_VIEW` 门禁与此无关。
//! * 排序走白名单、分页有上界、检索词只做 ILIKE 绑定，绝不拼原始输入。
//! * 返回字段里没有 email / passkey / totp（见仓储层三条硬口径）。

use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::repo::community::user_directory as repo_dir;
use crate::state::AppState;

/// 每页条数上限：与 `/admin/users` 的 per_page 上界一致。
const PER_MAX: i64 = 50;
const PER_DEFAULT: i64 = 20;

#[derive(Deserialize)]
struct UsersQuery {
    /// 用户名模糊检索（绑定进 SQL，不拼接）
    #[serde(default)]
    q: Option<String>,
    /// 按等级档筛（user_classes.id）
    #[serde(default)]
    class_id: Option<i32>,
    /// id|class|uploaded|downloaded|spark|last_seen|seeding|created
    #[serde(default)]
    sort: Option<String>,
    #[serde(default)]
    desc: Option<bool>,
    #[serde(default)]
    page: Option<i64>,
    #[serde(default)]
    per: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/v1/users",
    tag = "用户",
    description = "会员列表（登录可见；不含邮箱等敏感字段）。",
    params(
        ("q" = Option<String>, description = "用户名模糊检索"),
        ("class_id" = Option<i32>, description = "等级档筛选"),
        ("sort" = Option<String>, description = "排序列白名单"),
        ("page" = Option<i64>, description = "页码，1 起"),
        ("per" = Option<i64>, description = "每页 1-50，缺省 20"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/users")]
pub(crate) async fn users_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<UsersQuery>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    let per = q.per.unwrap_or(PER_DEFAULT).clamp(1, PER_MAX);
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let search =
        q.q.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.chars().take(64).collect::<String>());
    let order = repo_dir::order_col(q.sort.as_deref());
    let desc = q.desc.unwrap_or(true);
    let rows = repo_dir::members(
        &state.repo,
        search.as_deref(),
        q.class_id,
        order,
        desc,
        per,
        crate::dto::page_offset(page, per),
    )
    .await?;
    let total =
        repo_dir::count_members(&state.repo, search.as_deref(), q.class_id)
            .await?;
    Ok(ok(serde_json::json!({
        "rows": rows,
        "total": total,
        "page": page,
        "per": per,
        "sort": order,
        "desc": desc,
    })))
}
