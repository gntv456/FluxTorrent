//! 运维 webhook 的管理面（0389）。
//!
//! 三屏：出网白名单 / 出口健康度 / 投递台账。
//!
//! 【为什么要管理面而不是只留代码里的护栏】护栏会自动拒内网，但站长
//! 站点里挂内网 webhook 是合法需求（Discord 走公网，但自建告警接收端
//! 常在内网）。没有登记入口，站长就只能把 URL 改成公网中转，白白多一跳
//! 且多一份凭据泄露面。
//!
//! 签名密钥的读写都走这里：读一律掩码（它是凭据），写只接受「重新生成」
//! 或手工填入，不回显明文。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::authz::perm;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::repo::webhook as repo_wh;
use crate::state::AppState;

/// 出网白名单 + 出口健康度（管理面第一屏）。
#[get("/admin/webhooks")]
pub(crate) async fn list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::SETTINGS_MANAGE).await?;
    let repo = &state.repo;
    let cidrs = repo_wh::egress_cidrs(repo).await?;
    let threshold: i64 = repo_wh::setting(repo, "webhook_failure_threshold")
        .await
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let mut channels = vec![];
    for c in ["discord", "telegram"] {
        let fails = repo_wh::consecutive_failures(repo, c).await?;
        channels.push(serde_json::json!({
            "channel": c,
            "consecutive_failures": fails,
            "disabled": fails >= threshold && threshold > 0,
            "configured": !repo_wh::setting(repo, if c == "discord" {
                "webhook_discord"
            } else {
                "tg_bot_token"
            })
            .await
            .unwrap_or_default()
            .is_empty(),
        }));
    }
    // 签名密钥只报「是否已配置」，绝不回显明文。
    let secret_set = !repo_wh::setting(repo, "webhook_sign_secret")
        .await
        .unwrap_or_default()
        .is_empty();
    Ok(ok(serde_json::json!({
        "egress_cidrs": cidrs,
        "channels": channels,
        "signing_enabled": secret_set,
        "failure_threshold": threshold,
        "retry_times": repo_wh::setting(repo, "webhook_retry_times")
            .await.unwrap_or_default(),
    })))
}

#[derive(Deserialize)]
struct CidrBody {
    cidr: String,
    #[serde(default)]
    note: String,
}

/// 登记一条出网白名单网段。
#[post("/admin/webhooks/egress")]
pub(crate) async fn add_cidr(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CidrBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::SETTINGS_MANAGE).await?;
    let cidr = body.cidr.trim();
    if cidr.is_empty() {
        return Err(DomainError::Validation("网段不能为空".into()));
    }
    // 登记内网网段是高权限动作：它让 api 可以向该网段出网。
    if !cidr.contains('/') {
        return Err(DomainError::Validation(
            "请填 CIDR（含前缀长度），例如 192.168.10.0/24".into(),
        ));
    }
    let added = repo_wh::add_egress_cidr(&state.repo, cidr, body.note.trim())
        .await?;
    state.repo.audit(Some(auth.id), "webhook.egress_add", None).await;
    Ok(ok(serde_json::json!({ "added": added, "cidr": cidr })))
}

#[derive(Deserialize)]
struct DelBody {
    cidr: String,
}

/// 删除一条白名单网段。
#[post("/admin/webhooks/egress/delete")]
pub(crate) async fn del_cidr(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DelBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::SETTINGS_MANAGE).await?;
    let n = repo_wh::del_egress_cidr(&state.repo, body.cidr.trim()).await?;
    state
        .repo
        .audit(Some(auth.id), "webhook.egress_del", None)
        .await;
    Ok(ok(serde_json::json!({ "deleted": n })))
}

#[derive(Deserialize)]
struct ResetBody {
    channel: String,
}

/// 清掉某出口的失败计数（改完配置后手工恢复）。
#[post("/admin/webhooks/reset")]
pub(crate) async fn reset(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ResetBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::SETTINGS_MANAGE).await?;
    let ch = body.channel.trim().to_string();
    if !matches!(ch.as_str(), "discord" | "telegram") {
        return Err(DomainError::Validation("未知出口".into()));
    }
    let n = repo_wh::reset_failures(&state.repo, &ch).await?;
    state
        .repo
        .audit(Some(auth.id), "webhook.reset", None)
        .await;
    Ok(ok(serde_json::json!({ "cleared": n, "channel": ch })))
}

#[derive(Deserialize)]
struct LedgerQuery {
    #[serde(default = "default_limit")]
    limit: i64,
}
fn default_limit() -> i64 {
    50
}

/// 投递台账（「告警到底送到没有」从此可查）。
#[get("/admin/webhooks/deliveries")]
pub(crate) async fn deliveries(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<LedgerQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::SETTINGS_MANAGE).await?;
    let rows = repo_wh::recent_deliveries(&state.repo, q.limit.clamp(1, 200))
        .await?;
    Ok(ok(serde_json::json!({ "deliveries": rows })))
}
