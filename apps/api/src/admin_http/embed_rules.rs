//! 论坛视频内嵌 V1（0189）：embed 规则后台 CRUD。
//!
//! 写入口安全：正则保存前编译校验 + 必须域名字面量锚定（防 ReDoS/逃逸）、
//! 模板 $n 引用数 ≤ 捕获组数、embed_template 生成物必须落在 embed_origin
//! 前缀内（与渲染层同一条防线，坏配置在保存时即拦截）。
//! builtin 规则可停用不可删（0176 口径）。

use actix_web::{get, post, put, web, HttpRequest, HttpResponse};

use super::embed_rule_check::{validate_body, EmbedRuleBody};
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
    // 规则影响首页公告的 iframe/video src 白名单 → 失效首页共享段缓存
    crate::http::invalidate_home_cache(&state).await;
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
    crate::http::invalidate_home_cache(&state).await;
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
    crate::http::invalidate_home_cache(&state).await;
    Ok(ok(serde_json::json!({ "deleted": true })))
}
