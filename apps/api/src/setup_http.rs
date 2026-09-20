//! 安装向导（U3 §8.3）：冷启动检测 → 三步向导 → setup_done 置位。
//!
//! 未完成时封锁一切业务 API（防裸奔站被扫），仅放行：
//! /setup*、/health、/auth/login（站长可能需要先登录）、静态兼容探针 /compat/meta。
//! 向导完成动作：purge demo → 应用站型包 extras → 建管理员（如未建）→ 置 setup_done。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 业务 API 封锁中间件（module_gate 之前判断）：
/// setup_done 未置位且路径不在白名单 → 403 引导向 /setup。
pub async fn setup_gate_mw(
    req: actix_web::dev::ServiceRequest,
    next: actix_web::middleware::Next<impl actix_web::body::MessageBody>,
) -> actix_web::Result<actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>> {
    let path = req.path();
    const ALLOW: &[&str] = &[
        "/api/v1/setup",
        "/api/v1/setup/status",
        "/api/v1/health",
        "/api/v1/auth/login",
        "/api/v1/compat/meta",
        "/api/v1/metrics",
    ];
    if !path.starts_with("/api/v1") || ALLOW.iter().any(|p| path.starts_with(p)) {
        return next.call(req).await;
    }
    let state = req
        .app_data::<web::Data<std::sync::Arc<AppState>>>()
        .cloned()
        .expect("AppState registered");
    let done: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = 'setup_done'), '')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_default();
    if done == "done" {
        return next.call(req).await;
    }
    // FLUX_DEMO=1 演示环境跳过封锁（演示站要能直接看）
    if std::env::var("FLUX_DEMO").unwrap_or_default() == "1" {
        return next.call(req).await;
    }
    Err(actix_web::Error::from(DomainError::Validation(
        "站点尚未完成安装向导，请先访问 /setup".into(),
    )))
}

/// 向导状态（公开）
#[get("/setup/status")]
async fn setup_status(state: web::Data<std::sync::Arc<AppState>>) -> impl Responder {
    let done: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = 'setup_done'), '')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_default();
    let packs: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT code, name, COALESCE(description, '') FROM site_type_packs ORDER BY sort",
    )
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    let has_admin: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE class_id = 99)")
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
    ok(serde_json::json!({
        "done": done == "done",
        "has_admin": has_admin,
        "packs": packs.into_iter().map(|(code, name, descr)| {
            serde_json::json!({"code": code, "name": name, "description": descr})
        }).collect::<Vec<_>>(),
    }))
}

#[derive(Deserialize)]
struct SetupFinishBody {
    /// 所选站型包 code（空 = 保持当前）
    #[serde(default)]
    pack: String,
    /// 站名（空 = 不改）
    #[serde(default)]
    site_name: String,
    /// 玩法合规确认（§11.3：必须显式 true 才允许完成，留审计）
    games_compliance_ack: bool,
}

/// 完成向导（sysop）：purge demo → 应用包 → 置位。幂等可重入。
#[post("/setup")]
async fn setup_finish(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SetupFinishBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SITEPACKS_MANAGE).await?;
    if !body.games_compliance_ack {
        return Err(DomainError::Validation(
            "需确认已了解娱乐玩法（机会类游戏）的本地合规要求".into(),
        ));
    }
    // 1) demo 清理（幂等：重复执行零行）
    let purged: Vec<(String, i64)> = sqlx::query_as("SELECT kind, removed FROM purge_demo_data()")
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 2) 站型应用（extras：等级/经济/元数据；分类由既有 apply 端点处理，向导引导先调）
    let mut extras_applied: Vec<(String, i64)> = Vec::new();
    if !body.pack.is_empty() {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM site_type_packs WHERE code = $1)")
                .bind(&body.pack)
                .fetch_one(&state.repo.db)
                .await
                .unwrap_or(false);
        if !exists {
            return Err(DomainError::Validation("站型包不存在".into()));
        }
        extras_applied = sqlx::query_as("SELECT kind, applied FROM apply_pack_extras($1)")
            .bind(&body.pack)
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let _ = sqlx::query(
            "UPDATE site_settings SET value = $2, updated_at = now() WHERE name = 'site_type'",
        )
        .bind(&body.pack)
        .execute(&state.repo.db)
        .await;
    }
    // 3) 站名（可选）
    if !body.site_name.trim().is_empty() {
        let _ = sqlx::query(
            "UPDATE site_settings SET value = $2, updated_at = now() WHERE name = 'site_name'",
        )
        .bind(body.site_name.trim())
        .execute(&state.repo.db)
        .await;
    }
    // 4) 置位（幂等）
    sqlx::query(
        "INSERT INTO site_settings (name, value) VALUES ('setup_done', 'done') \
         ON CONFLICT (name) DO UPDATE SET value = 'done', updated_at = now()",
    )
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.module_flags.invalidate().await;
    state.repo.audit(Some(auth.id), "setup_finish", None).await;
    Ok(ok(serde_json::json!({
        "done": true,
        "purged": purged,
        "extras": extras_applied,
        "compliance_ack": true,
    })))
}

pub fn mount_setup(scope: actix_web::Scope) -> actix_web::Scope {
    scope.service(setup_status).service(setup_finish)
}
