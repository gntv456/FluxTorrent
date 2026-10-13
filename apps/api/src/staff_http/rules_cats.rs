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
    let n = sqlx::query("DELETE FROM site_rules WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    // P-229：旧写法丢掉 rows_affected ⇒ 删不存在的 id 也回 {"ok":true}，
    // 并照写一条「删过 99xxxx」的假审计。
    if n == 0 {
        return Err(DomainError::NotFound(*path as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "rules.delete", Some(*path as i64))
        .await;
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
