//! Wiki 最小落地（0359）：wiki_pages 后台 CRUD + 版本史。
//! 公开读端点在 wiki_public.rs（300 行门禁拆分）。
//!
//! 与 custom_pages 的关键差异：正文是 BBCode——前端 renderBBCode 渲染成
//! React 节点，天然免疫 XSS，出站**不做** ammonia 消毒（custom_pages 是
//! 管理员 HTML、注水注定的，才需要那道防线）。
//! 版本化走 rules_revisions（0079）口径：全量快照不存 diff，PUT 同事务
//! 先 INSERT 旧行进 wiki_revisions 再 UPDATE（rules_cats.rs 先例）。

use actix_web::{get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

/// 与 custom_pages 同口径：小写字母/数字/连字符，非空且 ≤60 字节
pub(crate) fn valid_wiki_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 60
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// 正文上限 200KB（与 custom_pages 同档）
pub(crate) const WIKI_BODY_LIMIT: usize = 200_000;

#[derive(Deserialize)]
pub(crate) struct WikiBody {
    slug: String,
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default = "default_true")]
    visible: bool,
    #[serde(default = "default_sort")]
    sort: i32,
}
fn default_true() -> bool {
    true
}
fn default_sort() -> i32 {
    100
}

pub(crate) fn validate_wiki_body(b: &WikiBody) -> DomainResult<()> {
    if !valid_wiki_slug(&b.slug) {
        return Err(DomainError::Validation(
            "slug 需为小写字母/数字/连字符，≤60 字符".into(),
        ));
    }
    if b.title.trim().is_empty() || b.title.len() > 100 {
        return Err(DomainError::Validation("标题需 1-100 字符".into()));
    }
    if b.body.len() > WIKI_BODY_LIMIT {
        return Err(DomainError::Validation("正文超过 200KB 上限".into()));
    }
    Ok(())
}

async fn require_wiki_manage(
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
        "SELECT id, slug, title, body, visible, sort, updated_at \
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
         edited_by) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(body.slug.trim())
    .bind(body.title.trim())
    .bind(&body.body)
    .bind(body.visible)
    .bind(body.sort)
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
        .audit(Some(auth.id), "wiki_create", Some(id))
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
         visible = $5, sort = $6, edited_by = $7, updated_at = now() \
         WHERE id = $1",
    )
    .bind(*path)
    .bind(body.slug.trim())
    .bind(body.title.trim())
    .bind(&body.body)
    .bind(body.visible)
    .bind(body.sort)
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
        .audit(Some(auth.id), "wiki_update", Some(*path))
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
    let n = sqlx::query("DELETE FROM wiki_pages WHERE id = $1")
        .bind(path.into_inner())
        .execute(&state.repo.db)
        .await
        .map_err(internal)?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("页面不存在".into()));
    }
    state.repo.audit(Some(auth.id), "wiki_delete", None).await;
    Ok(ok(serde_json::json!({ "deleted": true })))
}

/// GET /admin/wiki/{id}/revisions：版本历史（照 v4_http rule_revisions，
/// LIMIT 50 新况在前；只读，回滚不做——最小落地）
#[get("/admin/wiki/{id}/revisions")]
pub async fn wiki_revisions(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_wiki_manage(&state, &auth).await?;
    let rows: Vec<WikiRevisionRow> = sqlx::query_as(
        "SELECT r.id, r.page_id, r.title, r.body, r.sort, \
         u.username AS edited_by_name, r.created_at \
         FROM wiki_revisions r LEFT JOIN users u ON u.id = r.edited_by \
         WHERE r.page_id = $1 ORDER BY r.created_at DESC LIMIT 50",
    )
    .bind(*path)
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    Ok(ok(rows))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct WikiRevisionRow {
    id: i64,
    page_id: i64,
    title: String,
    body: String,
    sort: i32,
    edited_by_name: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wb(slug: &str, title: &str, body: &str) -> WikiBody {
        WikiBody {
            slug: slug.into(),
            title: title.into(),
            body: body.into(),
            visible: true,
            sort: 100,
        }
    }

    #[test]
    fn slug_rules_match_custom_pages() {
        assert!(valid_wiki_slug("a"));
        assert!(valid_wiki_slug("beginner-guide-2026"));
        assert!(valid_wiki_slug(&"x".repeat(60)));
        for bad in ["", "A", "Upper", "sp ace", "下划线_1", "dot.slug"] {
            assert!(!valid_wiki_slug(bad), "{bad} 不应通过");
        }
        assert!(!valid_wiki_slug(&"x".repeat(61)), "超 60 应拒绝");
    }

    #[test]
    fn body_over_200kb_rejected() {
        let big = "x".repeat(WIKI_BODY_LIMIT + 1);
        assert!(validate_wiki_body(&wb("ok", "t", &big)).is_err());
        assert!(validate_wiki_body(&wb("ok", "t", "")).is_ok());
    }

    #[test]
    fn title_bounds_enforced() {
        assert!(validate_wiki_body(&wb("ok", "", "b")).is_err());
        let long = "标".repeat(51); // 51×3 字节 > 100
        assert!(validate_wiki_body(&wb("ok", &long, "b")).is_err());
        assert!(validate_wiki_body(&wb("ok", "标题", "b")).is_ok());
    }
}
