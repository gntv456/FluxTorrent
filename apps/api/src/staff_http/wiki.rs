//! Wiki 后台 CRUD + 版本史（0359）。拆分账：公开读 wiki_public.rs、
//! 请求体校验与单测 wiki_validate.rs（0495 section 字段第三次顶线）。
//!
//! 与 custom_pages 的关键差异：正文是 BBCode——前端 renderBBCode 渲染成
//! React 节点，天然免疫 XSS，出站**不做** ammonia 消毒（custom_pages 是
//! 管理员 HTML、注水注定的，才需要那道防线）。
//! 版本化走 rules_revisions（0079）口径：全量快照不存 diff，PUT 同事务
//! 先 INSERT 旧行进 wiki_revisions 再 UPDATE（rules_cats.rs 先例）。

use actix_web::{get, post, put, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::wiki_validate::{validate_wiki_body, WikiBody};

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

pub(super) async fn require_wiki_manage(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &crate::http::AuthUser,
) -> DomainResult<()> {
    crate::authz::require_perm(state, auth, crate::authz::perm::WIKI_MANAGE)
        .await
}

// ============ 后台 CRUD ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct WikiPageRow {
    id: i64,
    slug: String,
    title: String,
    body: String,
    visible: bool,
    sort: i32,
    section: String,
    updated_at: chrono::DateTime<chrono::Utc>,
}

/// GET /admin/wiki：管理视角列表（含不可见页）
#[get("/admin/wiki")]
pub async fn wiki_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_wiki_manage(&state, &auth).await?;
    let rows: Vec<WikiPageRow> = sqlx::query_as(
        "SELECT id, slug, title, body, visible, sort, section, updated_at \
         FROM wiki_pages ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    Ok(ok(rows))
}

/// POST /admin/wiki：创建（首版不进 revisions——快照存的是「被替换的旧版」）
#[post("/admin/wiki")]
pub async fn wiki_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WikiBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_wiki_manage(&state, &auth).await?;
    validate_wiki_body(&body)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO wiki_pages (slug, title, body, visible, sort, \
         section, edited_by) VALUES ($1, $2, $3, $4, $5, $6, $7) \
         RETURNING id",
    )
    .bind(body.slug.trim())
    .bind(body.title.trim())
    .bind(&body.body)
    .bind(body.visible)
    .bind(body.sort)
    .bind(body.section.trim())
    .bind(auth.id)
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
        .audit_detail(
            Some(auth.id),
            "wiki_create",
            Some(id),
            None,
            Some(serde_json::json!({
                "slug": body.slug.trim(), "title": body.title.trim()
            })),
        )
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

/// PUT /admin/wiki/{id}：同事务先存档旧行进 wiki_revisions 再 UPDATE
/// （rules_cats.rs:67-97 的事务存档模式——快照与更新要么都发生要么都不）
#[put("/admin/wiki/{id}")]
pub async fn wiki_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<WikiBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_wiki_manage(&state, &auth).await?;
    validate_wiki_body(&body)?;
    let mut tx = state.repo.db.begin().await.map_err(internal)?;
    sqlx::query(
        "INSERT INTO wiki_revisions (page_id, title, body, sort, \
         edited_by) SELECT id, title, body, sort, $2 \
         FROM wiki_pages WHERE id = $1",
    )
    .bind(*path)
    .bind(auth.id)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
    let n = sqlx::query(
        "UPDATE wiki_pages SET slug = $2, title = $3, body = $4, \
         visible = $5, sort = $6, section = $7, edited_by = $8, \
         updated_at = now() WHERE id = $1",
    )
    .bind(*path)
    .bind(body.slug.trim())
    .bind(body.title.trim())
    .bind(&body.body)
    .bind(body.visible)
    .bind(body.sort)
    .bind(body.section.trim())
    .bind(auth.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            DomainError::Validation("slug 已存在".into())
        }
        _ => internal(e),
    })?
    .rows_affected();
    tx.commit().await.map_err(internal)?;
    if n == 0 {
        return Err(DomainError::Validation("页面不存在".into()));
    }
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "wiki_update",
            Some(*path),
            None,
            Some(serde_json::json!({
                "slug": body.slug.trim(), "title": body.title.trim()
            })),
        )
        .await;
    Ok(ok(serde_json::json!({ "updated": true })))
}

/// DELETE /admin/wiki/{id}：revisions 经 ON DELETE CASCADE 连带清除
#[post("/admin/wiki/{id}/delete")]
pub async fn wiki_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_wiki_manage(&state, &auth).await?;
    let pid = path.into_inner();
    // 先读回对象：revisions 随 ON DELETE CASCADE 一起没了，删完再问
    // 「删的是哪一页、谁删的」就完全没有出处
    let gone: Option<(String, String)> =
        sqlx::query_as("SELECT slug, title FROM wiki_pages WHERE id = $1")
            .bind(pid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(internal)?;
    let (slug, title) = match gone {
        Some(t) => t,
        None => return Err(DomainError::Validation("页面不存在".into())),
    };
    let n = sqlx::query("DELETE FROM wiki_pages WHERE id = $1")
        .bind(pid)
        .execute(&state.repo.db)
        .await
        .map_err(internal)?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("页面不存在".into()));
    }
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "wiki_delete",
            Some(pid),
            None,
            Some(serde_json::json!({ "slug": slug, "title": title })),
        )
        .await;
    Ok(ok(serde_json::json!({ "deleted": true })))
}
