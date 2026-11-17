//! 开放 API：本人等级门槛与四维现状（Token 鉴权）。
//!
//! 为什么要开放 API 侧再来一份：页面口径的 `/classes`（gaps 域）认登录态
//! JWT，而插件生态（PT-Plugin-Plus / PT-depiler 用户卡）手上只有开放 API
//! Token（fxo_），拿不到等级门槛 —— 于是「等级升级规则」这一列对插件恒为空。
//! 这里按 UNIT3D `/groups/requirements` 口径开放同一份数据（read scope 即可）。
//!
//! SQL 在 R 层（repo::user_stats），H 层只调函数。

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::repo::user_stats;
use crate::state::AppState;
use actix_web::{get, web, HttpRequest, HttpResponse};

use super::{require_scope, require_token, with_rl, SCOPE_READ};

/// GET /open/me/classes —— 等级门槛 + 本人现状。
#[utoipa::path(
    get,
    path = "/api/v1/open/me/classes",
    tag = "我的数据",
    description = "等级升级规则（门槛四维）+ 本人四维现状；与页面 /classes 同口径。",
    responses(
        (status = 200, description = "classes/staff_classes/me_class/me_stats"),
        (status = 401, description = "Token 无效/已撤销/未传（code 2001）"),
        (status = 403, description = "缺所需 scope（code 2003）"),
        (status = 429, description = "限流（code 1015），带 Retry-After: 60"),
    )
)]
#[get("/open/me/classes")]
pub(super) async fn me_classes(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    require_scope(&tk, SCOPE_READ)?;
    let uid = tk.uid;
    let classes = user_stats::level_rules(&state.repo).await;
    let staff_classes = user_stats::staff_classes(&state.repo).await;
    let me_class = user_stats::my_class(&state.repo, uid).await;
    let me_stats = user_stats::my_stats(&state.repo, uid).await;
    Ok(with_rl(
        ok(serde_json::json!({
            "me_class": me_class,
            "me_stats": me_stats,
            "classes": classes,
            "staff_classes": staff_classes
                .into_iter()
                .map(|(id, name)| serde_json::json!({ "id": id, "name": name }))
                .collect::<Vec<_>>(),
        })),
        &tk,
    ))
}
