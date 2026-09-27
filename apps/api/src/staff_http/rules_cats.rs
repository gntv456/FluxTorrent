//! 规则与分类管理（modrules/catmanage）。
//! 从 staff_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

#[get("/rules-content")]
pub async fn rules_content(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let rows: Vec<RuleRow> = sqlx::query_as(
        "SELECT id, title, body, sort FROM site_rules ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct RuleBody {
    title: String,
    body: String,
    #[serde(default)]
    sort: Option<i32>,
}

#[post("/admin/rules")]
pub async fn rule_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RuleBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::RULES_MANAGE)
        .await?;
    let id: i32 = sqlx::query_scalar(
                "INSERT INTO site_rules (title, body, sort) VALUES \
         ($1,$2,COALESCE($3,(SELECT max(sort)+1 FROM site_rules))) RETURNING id",
    ).bind(&body.title).bind(&body.body).bind(body.sort)
    .fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "rules.create", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/rules/{id}")]
pub async fn rule_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<RuleBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::RULES_MANAGE)
        .await?;
    // G5 规则版本化：同事务内先存档被替换的旧版，再更新
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO rules_revisions (rule_id, title, body, sort, edited_by) \
         SELECT id, title, body, sort, $2 FROM site_rules WHERE id = $1",
    )
    .bind(*path)
    .bind(auth.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let n = sqlx::query(
        "UPDATE site_rules SET title=$2, body=$3, \
     sort=COALESCE($4, sort), updated_at=now() WHERE id=$1",
    )
    .bind(*path)
    .bind(&body.title)
    .bind(&body.body)
    .bind(body.sort)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if n == 0 {
        return Err(DomainError::NotFound(*path as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "rules.update", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/rules/{id}")]
pub async fn rule_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::RULES_MANAGE)
        .await?;
    sqlx::query("DELETE FROM site_rules WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "rules.delete", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ---- 分类管理（catmanage）----

#[derive(Deserialize)]
struct CatBody {
    name: String,
    /// 图标键（0166）：前端 Icon 语义名（film/tv/music/…）；空 = 回落分类名首字
    #[serde(default)]
    icon_key: Option<String>,
    /// 分类色（0183）：`#rrggbb`；缺省或空 = 不改
    #[serde(default)]
    bg_color: Option<String>,
    /// 父分类（0188 层级）：None/0 = 顶级；触发器防环（≤8 层）
    #[serde(default)]
    parent_id: Option<i32>,
    #[serde(default)]
    sort: Option<i32>,
}

/// 只接受 `#rrggbb`：这个值最终进前端内联 style，库里也有同形 CHECK 兜底
fn is_hex_color(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 7 && b[0] == b'#' && b[1..].iter().all(|c| c.is_ascii_hexdigit())
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct CatRow {
    id: i32,
    name: String,
    /// 父分类（0188 层级）：NULL = 顶级
    parent_id: Option<i32>,
    mode_id: Option<i32>,
    auto_approve: bool,
    torrents: i64,
    icon_key: String,
    sort: i32,
    /// 分类色（0183）：#rrggbb，NULL = 用前端中性兜底
    bg_color: Option<String>,
}

#[get("/admin/categories")]
pub async fn category_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let rows: Vec<CatRow> = sqlx::query_as(
        "SELECT c.id, c.name, c.parent_id, c.mode_id, c.auto_approve, c.icon_key, c.sort, c.bg_color, (SELECT count(*) FROM torrents t WHERE t.category_id = c.id)::bigint AS torrents \
         FROM categories c ORDER BY c.sort, c.id",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[post("/admin/categories")]
pub async fn category_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CatBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO categories (id, name, parent_id, sort) \
     VALUES ((SELECT max(id)+1 FROM categories), $1, NULLIF($2, 0), \
     COALESCE($3, 100)) RETURNING id",
    )
    .bind(&body.name)
    .bind(body.parent_id.unwrap_or(0))
    .bind(body.sort)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 审计修复（P1）：分类增删改此前完全不写审计日志
    state.repo.audit(Some(auth.id), "category.create", Some(id as i64)).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/categories/{id}")]
pub async fn category_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<CatBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    if let Some(c) = body.bg_color.as_deref() {
        if !c.is_empty() && !is_hex_color(c) {
            return Err(DomainError::Validation(
                "分类色需为 #rrggbb 十六进制".into(),
            ));
        }
    }
    let n = sqlx::query(
        "UPDATE categories SET name=$2, icon_key=$3, \
         bg_color = COALESCE(NULLIF($4, ''), bg_color), \
         parent_id = COALESCE(NULLIF($5, 0), parent_id), \
         sort = COALESCE($6, sort) WHERE id=$1",
    )
    .bind(*path)
    .bind(&body.name)
    .bind(body.icon_key.clone().unwrap_or_default())
    .bind(body.bg_color.clone().unwrap_or_default())
    .bind(body.parent_id.unwrap_or(0))
    .bind(body.sort)
    .execute(&state.repo.db)
    .await
    .map_err(|e| crate::errors::db_to_domain(e, "分类"))?;
    if n.rows_affected() == 0 {
        return Err(DomainError::NotFound(*path as i64));
    }
    let cid = *path as i64;
    state.repo.audit(Some(auth.id), "category.update", Some(cid)).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/categories/{id}")]
pub async fn category_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let used: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents WHERE category_id=$1",
    )
    .bind(*path)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    if used > 0 {
        return Err(DomainError::Validation(
            "该分类下仍有种子，无法删除".into(),
        ));
    }
    sqlx::query("DELETE FROM categories WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let did = *path as i64;
    state.repo.audit(Some(auth.id), "category.delete", Some(did)).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ---- 封禁系统（bans）----

#[derive(serde::Serialize, sqlx::FromRow)]
pub(super) struct RuleRow {
    pub(super) id: i32,
    pub(super) title: String,
    pub(super) body: String,
    pub(super) sort: i32,
}
