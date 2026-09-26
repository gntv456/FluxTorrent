//! 自定义页面（一审 R4.4）：custom_pages 后台 CRUD + 公开展示端点。
//!
//! body 是管理员撰写的 HTML——展示侧 ammonia 白名单消毒（与公告同防线）；
//! 菜单挂接走既有 menu_items（url 填 /p/{slug}）。

use actix_web::{get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

fn valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 60
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

// ============ 后台 CRUD ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct CustomPageRow {
    id: i64,
    slug: String,
    title: String,
    body: String,
    visible: bool,
    sort: i32,
    module_key: Option<String>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/custom-pages")]
pub async fn custom_pages_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CUSTOMPAGES_MANAGE,
    )
    .await?;
    let rows: Vec<CustomPageRow> = sqlx::query_as(
        "SELECT id, slug, title, body, visible, sort, module_key, updated_at \
         FROM custom_pages ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct CustomPageBody {
    slug: String,
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default = "default_true")]
    visible: bool,
    #[serde(default = "default_sort")]
    sort: i32,
    /// 挂到某模块：该模块关闭时页面从前台下线（0199）
    #[serde(default)]
    module_key: Option<String>,
}
fn default_true() -> bool {
    true
}
fn default_sort() -> i32 {
    100
}

/// 空串按「不挂模块」处理（后台清空选择框提交的就是空串）
pub(super) fn norm_module_key(k: &Option<String>) -> Option<&str> {
    k.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

fn validate_body(b: &CustomPageBody) -> DomainResult<()> {
    if !valid_slug(&b.slug) {
        return Err(DomainError::Validation(
            "slug 需为小写字母/数字/连字符，≤60 字符".into(),
        ));
    }
    if b.title.trim().is_empty() || b.title.len() > 100 {
        return Err(DomainError::Validation("标题需 1-100 字符".into()));
    }
    if b.body.len() > 200_000 {
        return Err(DomainError::Validation("正文超过 200KB 上限".into()));
    }
    Ok(())
}

#[post("/admin/custom-pages")]
pub async fn custom_pages_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CustomPageBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CUSTOMPAGES_MANAGE,
    )
    .await?;
    validate_body(&body)?;
    crate::modules::require_known_module(
        &state.repo.db,
        norm_module_key(&body.module_key),
    )
    .await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO custom_pages \
         (slug, title, body, visible, sort, module_key) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(body.slug.trim())
    .bind(body.title.trim())
    .bind(&body.body)
    .bind(body.visible)
    .bind(body.sort)
    .bind(norm_module_key(&body.module_key))
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            DomainError::Validation("slug 已存在".into())
        }
        _ => internal(e),
    })?;
    state
        .repo
        .audit(Some(auth.id), "custom_page_add", None)
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/custom-pages/{id}")]
pub async fn custom_pages_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<CustomPageBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CUSTOMPAGES_MANAGE,
    )
    .await?;
    validate_body(&body)?;
    crate::modules::require_known_module(
        &state.repo.db,
        norm_module_key(&body.module_key),
    )
    .await?;
    let n = sqlx::query(
        "UPDATE custom_pages SET slug = $2, title = $3, body = $4, \
         visible = $5, sort = $6, module_key = $7, updated_at = now() \
         WHERE id = $1",
    )
    .bind(path.into_inner())
    .bind(body.slug.trim())
    .bind(body.title.trim())
    .bind(&body.body)
    .bind(body.visible)
    .bind(body.sort)
    .bind(norm_module_key(&body.module_key))
    .execute(&state.repo.db)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            DomainError::Validation("slug 已存在".into())
        }
        _ => internal(e),
    })?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("页面不存在".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "custom_page_update", None)
        .await;
    Ok(ok(serde_json::json!({ "updated": true })))
}

#[post("/admin/custom-pages/{id}/delete")]
pub async fn custom_pages_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CUSTOMPAGES_MANAGE,
    )
    .await?;
    let n = sqlx::query("DELETE FROM custom_pages WHERE id = $1")
        .bind(path.into_inner())
        .execute(&state.repo.db)
        .await
        .map_err(internal)?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("页面不存在".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "custom_page_delete", None)
        .await;
    Ok(ok(serde_json::json!({ "deleted": true })))
}

// ============ 公开展示 ============

/// GET /public-pages：可被前台访问的自定义页清单（sitemap 用；无登录态）。
/// 与 /pages/{slug} 同一条可见性判据——挂了模块键且模块关闭的页面不出现在这里，
/// 免得爬虫拿到一个必然 404 的地址。
#[get("/public-pages")]
pub async fn custom_pages_public_list(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let rows: Vec<(String, String)> =
        sqlx::query_as::<_, (String, String)>(&format!(
            "SELECT slug, updated_at::date::text FROM custom_pages \
             WHERE visible AND {} ORDER BY sort, id LIMIT 500",
            crate::modules::module_on_sql("custom_pages")
        ))
        .fetch_all(&state.repo.db)
        .await
        .map_err(internal)?;
    Ok(ok(
        rows.into_iter()
            .map(|(slug, updated)| {
                serde_json::json!({ "slug": slug, "updated_at": updated })
            })
            .collect::<Vec<_>>(),
    ))
}

/// GET /pages/{slug}：消毒后的页面内容（visible=false 或不存在 → 404 信封）
#[get("/pages/{slug}")]
pub async fn custom_page_public(
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let slug = path.into_inner();
    let row: Option<(String, String, String)> =
        sqlx::query_as::<_, (String, String, String)>(&format!(
            "SELECT title, body, updated_at::text FROM custom_pages \
             WHERE slug = $1 AND visible AND {}",
            crate::modules::module_on_sql("custom_pages")
        ))
        .bind(&slug)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(internal)?;
    let Some((title, body, updated)) = row else {
        return Err(DomainError::NotFound(0));
    };
    // 出站消毒（与公告 home.rs 同防线）：剥 script/事件属性/javascript: 协议
    let safe = ammonia::Builder::default().clean(&body).to_string();
    Ok(ok(serde_json::json!({
        "slug": slug, "title": title,
        "body": safe, "updated_at": updated,
    })))
}
