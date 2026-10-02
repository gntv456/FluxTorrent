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
    ok(serde_json::json!({ "status": "up", "service": "flux-api" }))
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
