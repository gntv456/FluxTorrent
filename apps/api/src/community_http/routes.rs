//! /api/v1 的路由表（2026-11-02 从 `mod.rs` 按功能拆出）。
//!
//! 拆出理由：这张表每加一个域就要长一行，而 `mod.rs` 已经顶到行数基线
//! （冻结文件只许瘦不许胖）；路由编排与「域内 handler 怎么组织」本来也是
//! 两件事，分开后新增路由只动本文件。
//!
//! 顺序纪律（原注释保留）：所有 `mount_*` 必须挂在**同一个** /api/v1
//! scope 上 —— actix 的 scope 是前缀独占匹配，在 App 上另开一个会把原有
//! 700+ 路由全打成 404；也不要插在链中间，会让先注册的服务整批失效。

use actix_web::web;

use super::mount_community;

pub fn configure(cfg: &mut web::ServiceConfig) {
    let scope = crate::setup_http::mount_setup(
        crate::economy_http::mount_economy(crate::http::v1_scope()),
    );
    // CSRF token 签发口（独立于登录，供前端按需续期/轮换）。
    // 挂在这里而不是在 App 上另开一个 /api/v1 scope：actix 的 scope
    // 是前缀独占匹配，另开一个会把原有 700+ 路由全打成 404。

    let scope = mount_community(scope);
    let scope = crate::games_http::mount_games(crate::ops_http::mount_ops(
        crate::content_http::mount_content(scope),
    ));
    let scope = crate::gacha_http::mount_gacha(scope);
    let scope = crate::admin_p3_http::mount_p3_tools(
        crate::admin_p2_http::mount_p2_tools(crate::admin_http::mount_admin(
            scope,
        )),
    );
    let scope = crate::settings_http::mount_settings(scope);
    let scope = crate::adapter_http::mount_adapters(scope);
    let scope = crate::push_http::mount_push(scope);
    let scope = crate::gaps_http::mount_gaps(scope);
    let scope = crate::rss_http::mount_rss(scope);
    let scope = crate::twofa_http::mount_twofa(scope);
    let scope = crate::compat_http::mount_compat(scope);
    let scope = crate::v4_http::mount_v4(scope);
    let scope = crate::openapi_http::mount_openapi(scope);
    // CSRF token 签发口挂在同一个 /api/v1 scope（链尾）。
    // 位置有讲究：**不要**在 App 上另开 /api/v1 scope（actix 的 scope
    // 是前缀独占匹配，另开会把原有 700+ 路由全打成 404——本批踩过）；
    // 也不要插在链中间（同样会让先注册的服务整批失效，只有最后
    // 配的那个能通）。链尾是最稳的位置：后续 mount_* 都接在它后面。
    let scope = scope.configure(crate::csrf::configure);
    cfg.service(scope);
}
