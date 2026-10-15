//! 术语治理后台面（0386）：键登记 / 状态迁移 / 版本 / 巡检。
//!
//! 评估报告 §2.4 说的「机制」就落在这里——没有这层界面，术语表会慢慢
//! 变成一座没人敢动的坟：没人知道有哪些键、哪些已废弃、改错了怎么回滚。
//!
//! 巡检的判据在 `domain::terms::audit`（含「假绿」防护），本文件只取数
//! 与落报告。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::authz::perm;
use crate::dto::ok;
use crate::domain::terms as tg;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::repo::terms_gov as repo_tg;
use crate::state::AppState;

#[derive(Deserialize)]
struct KeyUpsertReq {
    key: String,
    #[serde(default)]
    display_zh: String,
    #[serde(default = "default_category")]
    category: String,
}
fn default_category() -> String {
    "system".into()
}

#[derive(Deserialize)]
struct StatusReq {
    key: String,
    status: String,
}

/// 术语键清单（治理面第一屏）。
#[get("/admin/terms/keys")]
pub(crate) async fn keys_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::TERMS_MANAGE).await?;
    let keys = repo_tg::list_keys(&state.repo).await?;
    Ok(ok(serde_json::json!({ "keys": keys })))
}

/// 登记 / 更新一个术语键。
#[post("/admin/terms/keys")]
pub(crate) async fn key_upsert(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<KeyUpsertReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::TERMS_MANAGE).await?;
    // 键名会进 URL / Redis key / 前端字典，字符集由 domain 层限定
    tg::validate_key(&body.key)
        .map_err(|e| DomainError::Validation(e.0))?;
    repo_tg::upsert_key(
        &state.repo,
        body.key.trim(),
        body.display_zh.trim(),
        body.category.trim(),
    )
    .await?;
    state
        .repo
        .audit(Some(auth.id), "terms.key_upsert", None)
        .await;
    Ok(ok(serde_json::json!({ "key": body.key.trim() })))
}

/// 迁移键状态（active ⇄ deprecated → retired）。
///
/// 状态机判定在 `domain::terms::can_transition`，非法迁移在这里被拦下
/// 并给出原因——比如「想从 active 直接跳 retired」会被要求先过
/// deprecated 一档，避免一个刻意退役的键被顺手复活且不留痕。
#[post("/admin/terms/keys/status")]
pub(crate) async fn key_set_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<StatusReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::TERMS_MANAGE).await?;
    let to = tg::Status::parse(&body.status);
    // 现状从库里取（缺登记视为 active）
    let from = repo_tg::list_keys(&state.repo)
        .await?
        .into_iter()
        .find(|k| k.key == body.key)
        .map(|k| tg::Status::parse(&k.status))
        .unwrap_or(tg::Status::Active);
    if !tg::can_transition(from, to) {
        return Err(DomainError::Validation(format!(
            "不允许的状态迁移：{} → {}",
            from.as_str(),
            to.as_str()
        )));
    }
    let n = repo_tg::set_status(&state.repo, &body.key, to.as_str()).await?;
    if n == 0 {
        return Err(DomainError::NotFound(0));
    }
    state
        .repo
        .audit(
            Some(auth.id),
            &format!("terms.key_{}", to.as_str()),
            None,
        )
        .await;
    Ok(ok(serde_json::json!({
        "key": body.key,
        "status": to.as_str(),
    })))
}

#[derive(Deserialize)]
struct VersionQ {
    #[serde(default = "default_vlimit")]
    limit: i64,
}
fn default_vlimit() -> i64 {
    50
}

/// 版本列表（新到旧）。
#[get("/admin/terms/versions")]
pub(crate) async fn versions_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<VersionQ>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::TERMS_MANAGE).await?;
    let versions =
        repo_tg::list_versions(&state.repo, q.limit.clamp(1, 200)).await?;
    let cur = repo_tg::current_version(&state.repo).await?;
    Ok(ok(serde_json::json!({
        "versions": versions,
        "current_version": cur,
    })))
}

/// 跑一轮覆盖率巡检（落一份报告）。
///
/// 「真绿」的判据不是覆盖率高，而是**高覆盖且零退役引用**——
/// 见 `domain::terms::audit`。
#[post("/admin/terms/audit")]
pub(crate) async fn run_audit(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, perm::TERMS_MANAGE).await?;
    let c = repo_tg::audit_counts(&state.repo).await?;
    let refs = repo_tg::retired_refs(&state.repo).await?;
    let a = tg::audit(
        &tg::AuditInput {
            total: c.total,
            active: c.active,
            deprecated: c.deprecated,
            retired: c.retired,
            with_rule: c.with_rule,
        },
        refs.len(),
    );
    let id = repo_tg::push_audit(
        &state.repo,
        a.total_keys,
        a.active_keys,
        a.deprecated_keys,
        a.retired_keys,
        a.with_rule,
        a.coverage_pct,
        &refs,
        if a.healthy { "healthy" } else { "needs_attention" },
    )
    .await?;
    state
        .repo
        .audit(Some(auth.id), "terms.audit_run", None)
        .await;
    Ok(ok(serde_json::json!({
        "report_id": id,
        "total_keys": a.total_keys,
        "active_keys": a.active_keys,
        "deprecated_keys": a.deprecated_keys,
        "retired_keys": a.retired_keys,
        "with_rule": a.with_rule,
        "coverage_pct": a.coverage_pct,
        "healthy": a.healthy,
        "retired_refs": refs,
    })))
}
