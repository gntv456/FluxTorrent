//! 健康与就绪探针。
//! 从 http/auth_infra.rs 拆出（ZT81 2026-10-02）：原文件逼近 300 行门禁，
//! 「探依赖/探活」与「鉴权设施」本属不同域，按项目惯例按域拆分。
//!
//! 分级约定：
//!   * `/health` = liveness：只表明**进程活着**，不探依赖（供重启判定）。
//!   * `/ready`  = readiness：DB + Redis 任一不可用即 503（供 LB/编排导流判定）。

use actix_web::{get, web, HttpResponse, Responder};

use crate::dto::ok;
use crate::state::AppState;

#[get("/health")]
async fn health() -> impl Responder {
    ok(serde_json::json!({
        "status": "up",
        "service": "flux-api",
        // 版本来源 Cargo.toml（与 ghcr 镜像 tag 同源）；站长报障/检查更新对齐用
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// 公开版本页数据（NP aboutnexus 口径）：站名 + 版本 + 建站年，不探依赖、
/// 不要求登录——页尾「Powered by FluxTorrent」点进来第一屏就要能渲染。
#[get("/about")]
async fn about(state: web::Data<std::sync::Arc<AppState>>) -> impl Responder {
    let site_name: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'site_name'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_default();
    // 文档地址（0313 site_settings 化）：开源通用建站系统的每个部署
    // 都不该默认外链到上游作者的私人 wiki。站长可在后台改指到自己的
    // 帮助页；缺行回落上游官方文档（写死值仅作缺省，不再是唯一真相）。
    let docs_url: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'docs_url'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .filter(|v: &String| !v.trim().is_empty())
    .unwrap_or_else(|| "https://github.com/gntv456/FluxTorrent#readme".into());
    ok(serde_json::json!({
        "product": "FluxTorrent",
        "version": env!("CARGO_PKG_VERSION"),
        "site_name": site_name,
        // 开源主页（页尾 FluxTorrent 链接的跳转目标之一）
        "source_url": "https://github.com/gntv456/FluxTorrent",
        "docs_url": docs_url,
    }))
}

/// 就绪探针：DB 与 Redis 任一不可用即 503。
///
/// 此前只有 `/health`——静态返回 up、不探任何依赖，而 compose 的 healthcheck
/// 正是用它：DB/Redis 挂掉时 api 仍报 healthy，LB/反代继续导流 → 用户侧大面积
/// 500。
#[get("/ready")]
async fn ready(state: web::Data<std::sync::Arc<AppState>>) -> HttpResponse {
    let db_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.repo.db)
        .await
        .is_ok();
    let mut c = state.redis.clone();
    let redis_ok: bool = redis::cmd("PING")
        .query_async::<String>(&mut c)
        .await
        .map(|p| p == "PONG")
        .unwrap_or(false);
    let ok = db_ok && redis_ok;
    let body = serde_json::json!({
        "code": if ok { 0 } else { 5000 },
        "message": if ok { "ok" } else { "依赖不可用" },
        "request_id": uuid::Uuid::new_v4().to_string(),
        "data": { "db": db_ok, "redis": redis_ok },
    });
    if ok {
        HttpResponse::Ok().json(body)
    } else {
        HttpResponse::ServiceUnavailable().json(body)
    }
}
