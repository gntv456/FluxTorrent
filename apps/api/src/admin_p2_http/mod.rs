//! 第五轮 P2：置顶促销 / 自定义菜单 / 消息模板（参考站 Other 组口径）。
//! 按域拆分（300 行门禁）：置顶促销在 promos.rs，自定义菜单在 menus.rs，
//! 消息模板与索赔在 templates.rs；staff 助手与挂载留在此。

mod export_ops;
mod menus;
mod menus_crud;
mod menus_public;
mod promos;
mod templates;

use actix_web::{web, HttpRequest};
use export_ops::*;
use menus::*;
use menus_crud::*;
use menus_public::*;

use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::state::AppState;

use promos::*;
use templates::*;

pub(super) async fn staff(
    req: &HttpRequest,
    state: &web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<crate::http::AuthUser> {
    let auth = require_auth(req, state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_PANEL)
        .await?;
    Ok(auth)
}

// ---- 置顶促销（sticky-promotions）----

pub fn mount_p2_tools(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(admin_export_users)
        .service(admin_export_torrents)
        .service(sticky_promos_list)
        .service(sticky_promos_add)
        .service(sticky_promos_update)
        .service(sticky_promos_delete)
        .service(sticky_promos_public)
        .service(menu_items_list)
        .service(menu_items_add)
        .service(menu_items_update)
        .service(menu_items_delete)
        .service(menu_settings_update)
        .service(menu_items_public)
        .service(msg_templates_list)
        .service(msg_templates_update)
        .service(msg_templates_preview)
        .service(admin_claims)
        .service(admin_claim_release)
}
