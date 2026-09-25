//! 论坛视频内嵌 V1（0189）：embed 规则后台 CRUD。
//!
//! 写入口安全：正则保存前编译校验 + 必须域名字面量锚定（防 ReDoS/逃逸）、
//! 模板 $n 引用数 ≤ 捕获组数、embed_template 生成物必须落在 embed_origin
//! 前缀内（与渲染层同一条防线，坏配置在保存时即拦截）。
//! builtin 规则可停用不可删（0176 口径）。

use actix_web::{get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::guard::staff;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct EmbedRuleRow {
    id: i64,
    provider: String,
    name_zh: String,
    url_pattern: String,
    embed_template: String,
    embed_origin: String,
    render_kind: String,
    aspect: String,
    extra_params: Option<String>,
    builtin: bool,
    enabled: bool,
    sort: i32,
}

#[derive(Deserialize)]
struct EmbedRuleBody {
    provider: String,
    name_zh: String,
    url_pattern: String,
    embed_template: String,
    embed_origin: String,
    #[serde(default = "default_iframe")]
    render_kind: String,
    #[serde(default = "default_aspect")]
    aspect: String,
    #[serde(default)]
    extra_params: Option<String>,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default = "default_sort")]
    sort: i32,
}
fn default_iframe() -> String {
    "iframe".into()
}
fn default_aspect() -> String {
    "16:9".into()
}
fn default_true() -> bool {
    true
}
fn default_sort() -> i32 {
    100
}

/// url_pattern 必须以 ^https?:// + 域名字面量开头（或站内 ^/api/v1/ 锚定），
/// 杜绝开放正则匹配任意 URL 的情况。
fn anchored_domain(pattern: &str) -> bool {
    let rest = pattern
        .strip_prefix("^https?://")
        .or_else(|| pattern.strip_prefix("^https://"))
        .or_else(|| pattern.strip_prefix("^http://"));
    let Some(rest) = rest else {
        return pattern.starts_with("^/api/v1/");
    };
    // 域名字面量段：字母数字点连字符，允许 (www\.)? 形态的少量转义
    let head: String = rest
        .chars()
        .take_while(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '.' | '-' | '\\' | '(' | ')')
        })
        .collect();
    !head.is_empty() && head.chars().any(|c| c.is_ascii_alphanumeric())
}

fn validate_body(b: &EmbedRuleBody) -> DomainResult<()> {
    if b.provider.trim().is_empty() || b.provider.len() > 40 {
        return Err(DomainError::Validation(
            "provider 需 1-40 字符".into(),
        ));
    }
    if b.name_zh.trim().is_empty() || b.name_zh.len() > 60 {
        return Err(DomainError::Validation("展示名需 1-60 字符".into()));
    }
    if !matches!(b.render_kind.as_str(), "iframe" | "video") {
        return Err(DomainError::Validation(
            "render_kind 只允许 iframe/video".into(),
        ));
    }
    if !matches!(b.aspect.as_str(), "16:9" | "4:3" | "1:1") {
        return Err(DomainError::Validation(
            "aspect 只允许 16:9/4:3/1:1".into(),
        ));
    }
    if b.url_pattern.len() > 500 || b.embed_template.len() > 1000 {
        return Err(DomainError::Validation("正则/模板超长".into()));
    }
    if !anchored_domain(&b.url_pattern) {
        return Err(DomainError::Validation(
            "url_pattern 须 ^https?:// 域名字面量锚定（或 ^/api/v1/ 站内）".into(),
        ));
    }
    // 正则可编译 + 捕获组清点（渲染侧换行不敏感，统一多行模式编译）
    let re = regex::Regex::new(&b.url_pattern).map_err(|e| {
        DomainError::Validation(format!("url_pattern 正则无效: {e}"))
    })?;
    // ReDoS 兜底：限长 + 禁嵌套量词的保守检查（catastrophic 回溯常见形态）
    if b.url_pattern.contains("(.*)") || b.url_pattern.contains("(.+)") {
        return Err(DomainError::Validation(
            "url_pattern 不允许 (.*)/(.+) 开放捕获".into(),
        ));
    }
    // 模板 $n 引用必须 ≤ 捕获组数；生成物必须落在 embed_origin 前缀内
    let groups = re.captures_len() - 1;
    for i in 1..=9 {
        if b.embed_template.contains(&format!("${i}")) && i > groups {
            return Err(DomainError::Validation(format!(
                "模板 ${i} 超出捕获组数（{groups}）"
            )));
        }
    }
    if !b.embed_template.starts_with(&b.embed_origin) {
        return Err(DomainError::Validation(
            "embed_template 必须以 embed_origin 开头".into(),
        ));
    }
    if let Some(extra) = &b.extra_params {
        if extra.len() > 200 || extra.contains(' ') {
            return Err(DomainError::Validation(
                "extra_params 需 ≤200 字符且不含空格".into(),
            ));
        }
    }
    Ok(())
}

#[get("/admin/embed-rules")]
pub async fn embed_rules_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    let rows: Vec<EmbedRuleRow> = sqlx::query_as(
        "SELECT id, provider, name_zh, url_pattern, embed_template, \
         embed_origin, render_kind, aspect, extra_params, builtin, \
         enabled, sort FROM video_embed_rules ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    Ok(ok(rows))
}

#[post("/admin/embed-rules")]
pub async fn embed_rules_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<EmbedRuleBody>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    validate_body(&body)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO video_embed_rules (provider, name_zh, url_pattern, \
         embed_template, embed_origin, render_kind, aspect, extra_params, \
         enabled, sort) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING id",
    )
    .bind(body.provider.trim())
    .bind(body.name_zh.trim())
    .bind(&body.url_pattern)
    .bind(&body.embed_template)
    .bind(&body.embed_origin)
    .bind(&body.render_kind)
    .bind(&body.aspect)
    .bind(&body.extra_params)
    .bind(body.enabled)
    .bind(body.sort)
    .fetch_one(&state.repo.db)
    .await
    .map_err(internal)?;
    state
        .repo
        .audit(Some(auth.id), "embed_rule_add", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/embed-rules/{id}")]
pub async fn embed_rules_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<EmbedRuleBody>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    validate_body(&body)?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE video_embed_rules SET provider=$2, name_zh=$3, \
         url_pattern=$4, embed_template=$5, embed_origin=$6, render_kind=$7, \
         aspect=$8, extra_params=$9, enabled=$10, sort=$11 WHERE id=$1",
    )
    .bind(id)
    .bind(body.provider.trim())
    .bind(body.name_zh.trim())
    .bind(&body.url_pattern)
    .bind(&body.embed_template)
    .bind(&body.embed_origin)
    .bind(&body.render_kind)
    .bind(&body.aspect)
    .bind(&body.extra_params)
    .bind(body.enabled)
    .bind(body.sort)
    .execute(&state.repo.db)
    .await
    .map_err(internal)?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("规则不存在".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "embed_rule_update", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "updated": true })))
}

#[post("/admin/embed-rules/{id}/delete")]
pub async fn embed_rules_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FORUMS_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    // builtin 规则只可停用不可删（种子跟随版本升级）
    let n = sqlx::query(
        "DELETE FROM video_embed_rules WHERE id = $1 AND NOT builtin",
    )
    .bind(id)
    .execute(&state.repo.db)
    .await
    .map_err(internal)?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation(
            "规则不存在或为内置规则（内置规则请停用）".into(),
        ));
    }
    state
        .repo
        .audit(Some(auth.id), "embed_rule_delete", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "deleted": true })))
}
