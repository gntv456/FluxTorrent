//! 一次性邮箱域名库 admin API（0370 M4 注册治理第 ④ 道闸）。
//! 与 agent_rules 同构（站级配置须 SETTINGS_MANAGE）：
//!   GET  /admin/email-domains            列表（q 模糊 + limit）
//!   GET  /admin/email-domains/export     导出（可分享 JSON）
//!   POST /admin/email-domains/import     幂等导入（≤10 万条/批）
//!   POST /admin/email-domains/delete     按域名删除（可多个）
//!
//! 内置库（source='builtin'）随迁移 0370 seed；导入行 source='import'。
//! 域名归一化：小写、去首尾空白、允许粘贴整邮箱（取 @ 后段）、去掉
//! scheme/路径（`https://x.com/` → `x.com`）。命中判定在
//! auth_http/register_gates.rs（含子域后缀匹配）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::repo::email_domains as domain_repo;
use crate::state::AppState;

use super::guard::staff;

/// 单批导入上限：10 万条（规格口径）；再多请分批，避免单事务过长。
const IMPORT_MAX: usize = 100_000;

/// 归一化域名：`Ok(domain)` / `Err(原话)`。
fn normalize(raw: &str) -> Result<String, String> {
    let mut s = raw.trim().to_lowercase();
    if s.is_empty() {
        return Err("域名为空".into());
    }
    // 粘贴整邮箱 / URL 时取域名部分
    if let Some((_, d)) = s.rsplit_once('@') {
        s = d.to_string();
    }
    if let Some((_, rest)) = s.split_once("://") {
        s = rest.to_string();
    }
    if let Some((host, _)) = s.split_once('/') {
        s = host.to_string();
    }
    if let Some((host, _)) = s.split_once(':') {
        s = host.to_string();
    }
    let s = s.trim_start_matches('.').trim_end_matches('.').to_string();
    if s.is_empty() || !s.contains('.') {
        return Err(format!("「{raw}」不是有效域名"));
    }
    if s.chars().count() > 253 || s.chars().any(|c| c.is_whitespace()) {
        return Err(format!("「{raw}」不是有效域名"));
    }
    // 只允许字母数字、点、连字符（IDN 请转 punycode 后再导入）
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return Err(format!("「{raw}」含非法字符（IDN 请转 punycode）"));
    }
    Ok(s)
}

#[derive(Deserialize)]
struct ListQuery {
    q: Option<String>,
    limit: Option<i64>,
}

/// GET /admin/email-domains：列表（q 子串匹配，默认 200 条）。
#[get("/admin/email-domains")]
async fn email_domains_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<ListQuery>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // P-338（批一百五十四）：本读口此前只过粗闸 staff()，而同文件的写口与
    // 兄弟端点都要「SETTINGS_MANAGE」（class 99 起）。后果：class 91「发布员」能读
    // 邮箱域名策略读口全表，写却被 403——**读比写松**，属权限倒挂。
    // 取证：user_can(uid, "settings.manage") 对 class 91 返回 false，
    // 而 GET 这条仍回 200（实测 91 档拿到 email-domains 200 行、
    // roles-dict 5 行、agentrules 12 条）。
    // 修法：读口与写口对齐同一把权限键；语义上「读配置」与「改配置」同档。
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let needle = format!("%{}%", q.q.as_deref().unwrap_or("").trim());
    let limit = q.limit.unwrap_or(200).clamp(1, 1000);
    let rows = domain_repo::list_domains(&state.repo, &needle, limit)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let total = domain_repo::count_domains(&state.repo).await.unwrap_or(0);
    Ok(crate::openapi_http::no_store(ok(serde_json::json!({
        "items": rows,
        "total": total,
    }))))
}

/// GET /admin/email-domains/export：导出全部域名（纯域名数组，便于分享）。
#[get("/admin/email-domains/export")]
async fn email_domains_export(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let domains = domain_repo::export_domains(&state.repo)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(crate::openapi_http::no_store(ok(serde_json::json!({
        "format": "fluxtorrent.emaildomains.v1",
        "count": domains.len(),
        "domains": domains,
    }))))
}

#[derive(Deserialize)]
struct ImportBody {
    /// 域名数组（也接受整邮箱/URL，自动取域名）
    #[serde(default)]
    domains: Vec<String>,
    /// 或纯文本（换行/逗号/分号分隔），与 domains 合并
    #[serde(default)]
    text: String,
}

/// POST /admin/email-domains/import：幂等导入（已存在跳过），返回明细。
#[post("/admin/email-domains/import")]
async fn email_domains_import(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ImportBody>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let mut raw: Vec<String> = body.domains.clone();
    raw.extend(
        body.text
            .split(['\n', '\r', ',', ';', ' ', '\t'])
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string),
    );
    if raw.is_empty() {
        return Err(DomainError::Validation("没有可导入的域名".into()));
    }
    if raw.len() > IMPORT_MAX {
        return Err(DomainError::Validation(format!(
            "单次导入上限 {IMPORT_MAX} 条（收到 {}）",
            raw.len()
        )));
    }
    let mut valid: Vec<String> = Vec::new();
    let mut rejected: Vec<serde_json::Value> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (i, r) in raw.iter().enumerate() {
        match normalize(r) {
            Ok(d) => {
                if seen.insert(d.clone()) {
                    valid.push(d);
                }
            }
            Err(w) => {
                rejected.push(serde_json::json!({ "index": i, "reason": w }))
            }
        }
    }
    // 整批一事务（ON CONFLICT 幂等）在 repo 内完成；已存在计入 skipped
    let added = domain_repo::import_domains(&state.repo, &valid)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "emaildomain.import", Some(added))
        .await;
    Ok(ok(serde_json::json!({
        "added": added,
        "skipped": valid.len() as i64 - added,
        "invalid": rejected.len() as i64,
        "rejected": rejected,
    })))
}

#[derive(Deserialize)]
struct DeleteBody {
    #[serde(default)]
    domains: Vec<String>,
}

/// POST /admin/email-domains/delete：按域名删除（批量）。
#[post("/admin/email-domains/delete")]
async fn email_domains_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DeleteBody>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let list: Vec<String> = body
        .domains
        .iter()
        .filter_map(|d| normalize(d).ok())
        .collect();
    if list.is_empty() {
        return Err(DomainError::Validation("没有可删除的域名".into()));
    }
    let n = domain_repo::delete_domains(&state.repo, &list)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "emaildomain.delete", Some(n as i64))
        .await;
    Ok(ok(serde_json::json!({ "deleted": n })))
}
