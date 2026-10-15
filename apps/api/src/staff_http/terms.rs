//! 术语表后台 CRUD（0205 / 四审 L7）。
//!
//! 一张行表 + 三条端点：列 / 写（upsert）/ 删。写侧落库后**立刻** `terms::reload`，
//! 让改词在下一个请求就生效（不走 TTL：「改了看不到」那类死锁已踩过一次）。
//! 命中预览放在前端（`apply-terms.ts` 扫字典），因为只有 web 侧拿得到界面文案本体。
//!
//! 【0386 补的一条纪律：每次改动都留版本快照】
//! 本文件原先三条写路径（upsert / toggle / delete）各写一套 SQL、改完
//! 直接 reload，**从不留痕**。于是 `term_versions` 一直是一张空表——
//! 版本功能成了摆设，「改错一个词只能手工改回来」正是它要消灭的那件事。
//! 现在每次改动前先 `push_version` 落一份全量快照（改动前的形态），
//! 回滚入口见 `terms_gov.rs` 的 `/admin/terms/versions/{v}/restore`。
//!
//! SQL 一律下沉到 `repo/terms_gov.rs`：本层是 H 层，layer_guard 禁 sqlx；
//! 且「改动的唯一入口」要在 repo 收口，才谈得上「每次改动都留快照」。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::repo::terms_gov as repo_tg;
use crate::state::AppState;
use crate::terms;

#[get("/admin/terms")]
pub async fn terms_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TERMS_MANAGE)
        .await?;
    let rows = repo_tg::list_terms(&state.repo).await?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct TermBody {
    canonical: String,
    replacement: String,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    descr: Option<String>,
    #[serde(default = "default_sort")]
    sort: i32,
}
fn default_true() -> bool {
    true
}
fn default_sort() -> i32 {
    100
}

/// 写一条规则（canonical 存在即覆盖）。
#[post("/admin/terms")]
pub async fn terms_upsert(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TermBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TERMS_MANAGE)
        .await?;
    let from = body.canonical.trim();
    let to = body.replacement.trim();
    if !terms::valid_term_word(from) || !terms::valid_term_word(to) {
        return Err(DomainError::Validation(
            "词条需 1-20 字符且不含花括号（{n}/{magic} 是插值占位符）".into(),
        ));
    }
    if from == to {
        return Err(DomainError::Validation(
            "规范词与替换词相同：这条规则什么都不会改".into(),
        ));
    }
    let live = repo_tg::active_term_count(&state.repo).await?;
    let known = repo_tg::term_exists(&state.repo, from).await?;
    if !known && body.enabled && live >= terms::MAX_RULES {
        return Err(DomainError::Validation(format!(
            "术语规则上限 {} 条（错误出口按条数线性扫描文案）",
            terms::MAX_RULES
        )));
    }
    // 先留快照（记的是**改动前**的形态），再改——顺序反了就回不去了。
    repo_tg::push_version(
        &state.repo,
        "staff",
        &format!("upsert {from}"),
        1,
        Some(auth.id),
    )
    .await?;
    repo_tg::upsert_term(
        &state.repo,
        from,
        to,
        body.enabled,
        body.descr.as_deref().unwrap_or("").trim(),
        body.sort,
    )
    .await?;
    let n = terms::reload(&state.repo.db).await;
    crate::cfgver::bump(state.get_ref(), "terms").await;
    Ok(ok(serde_json::json!({
        "canonical": from,
        "replacement": to,
        "enabled_rules": n,
    })))
}

/// 停用/启用一条规则（保留词条，只切 enabled）。
#[post("/admin/terms/{canonical}/toggle")]
pub async fn terms_toggle(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TERMS_MANAGE)
        .await?;
    let key = path.as_str();
    // 词条不存在就没得「切」——先确认，避免给不存在的词记一份空快照。
    if !repo_tg::term_exists(&state.repo, key).await? {
        return Err(DomainError::Validation("没有这条术语规则".into()));
    }
    repo_tg::push_version(
        &state.repo,
        "staff",
        &format!("toggle {key}"),
        1,
        Some(auth.id),
    )
    .await?;
    let enabled = repo_tg::toggle_term(&state.repo, key)
        .await?
        .ok_or_else(|| DomainError::Validation("没有这条术语规则".into()))?;
    terms::reload(&state.repo.db).await;
    crate::cfgver::bump(state.get_ref(), "terms").await;
    Ok(ok(serde_json::json!({ "enabled": enabled })))
}

/// 删一条规则。confirm 字段与自定义页面/字段面板同形（危险操作要显式确认）。
#[derive(Deserialize)]
struct TermDeleteBody {
    #[serde(default)]
    confirm: bool,
}

#[post("/admin/terms/{canonical}/delete")]
pub async fn terms_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<TermDeleteBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TERMS_MANAGE)
        .await?;
    if !body.confirm {
        return Err(DomainError::Validation("请确认删除".into()));
    }
    let key = path.as_str();
    // 删不存在的词不算改动，别为它记快照（否则版本列表会被无意义行淹没）。
    if !repo_tg::term_exists(&state.repo, key).await? {
        return Err(DomainError::Validation("没有这条术语规则".into()));
    }
    repo_tg::push_version(
        &state.repo,
        "staff",
        &format!("delete {key}"),
        1,
        Some(auth.id),
    )
    .await?;
    let n = repo_tg::delete_term(&state.repo, key).await?;
    terms::reload(&state.repo.db).await;
    crate::cfgver::bump(state.get_ref(), "terms").await;
    Ok(ok(serde_json::json!({ "deleted": n })))
}
