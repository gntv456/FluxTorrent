//! 过审自动促销规则管理面（0503 运营台账 #5）：
//! auto_promo_rules（0077）在 review_side_effects 消费、命中即自动挂
//! 促销——但此前无任何 admin 写口，配置只能 SQL 打库。补全 CRUD
//! （SETTINGS_MANAGE 权限，与促销面板同档）。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::meta::staff;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct AutoRuleReq {
    pub name: String,
    pub name_regex: Option<String>,
    pub min_size: Option<i64>,
    pub max_size: Option<i64>,
    pub category_id: Option<i32>,
    pub kind: String,
    pub hours: i32,
    pub enabled: Option<bool>,
}

/// GET /admin/auto-promo-rules —— 全量规则（按 position 排序）。
#[get("/admin/auto-promo-rules")]
pub async fn auto_rules_list(
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
    let rows: Vec<(
        i64,
        String,
        String,
        i64,
        i64,
        Option<i32>,
        String,
        i32,
        bool,
    )> = sqlx::query_as(
        "SELECT id, name, name_regex, min_size, max_size, category_id, \\
             kind, hours, enabled FROM auto_promo_rules \\
             ORDER BY position, id LIMIT 100",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!(rows
        .iter()
        .map(|(id, name, rx, mn, mx, cat, kind, hrs, en)| {
            serde_json::json!({
                "id": id, "name": name, "name_regex": rx,
                "min_size": mn, "max_size": mx,
                "category_id": cat, "kind": kind,
                "hours": hrs, "enabled": en,
            })
        })
        .collect::<Vec<_>>())))
}

/// POST /admin/auto-promo-rules —— 新建规则。
#[post("/admin/auto-promo-rules")]
pub async fn auto_rules_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AutoRuleReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("规则名不能为空".into()));
    }
    let valid_kind = matches!(
        body.kind.as_str(),
        "free" | "x2" | "x2free" | "half" | "x2half" | "p30"
    );
    if !valid_kind {
        return Err(DomainError::Validation(
            "kind 仅支持 free/x2/x2free/half/x2half/p30".into(),
        ));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO auto_promo_rules \\
             (name, name_regex, min_size, max_size, category_id, kind, \\
              hours, enabled, position) \\
         VALUES ($1, COALESCE($2, ''), COALESCE($3, 0), COALESCE($4, 0), \\
              $5, $6, $7, COALESCE($8, TRUE), \\
              (SELECT COALESCE(max(position), 0) + 1 FROM auto_promo_rules)) \\
         RETURNING id",
    )
    .bind(body.name.trim())
    .bind(&body.name_regex)
    .bind(body.min_size.unwrap_or(0))
    .bind(body.max_size.unwrap_or(0))
    .bind(body.category_id)
    .bind(&body.kind)
    .bind(body.hours)
    .bind(body.enabled)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "auto_rule.add", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

/// PUT /admin/auto-promo-rules/{id} —— 编辑（含启停）。
#[put("/admin/auto-promo-rules/{id}")]
pub async fn auto_rules_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<AutoRuleReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    sqlx::query(
        "UPDATE auto_promo_rules SET name = $2, name_regex = COALESCE($3, ''), \
           min_size = COALESCE($4, min_size), \
           max_size = COALESCE($5, max_size), \
           category_id = $6, kind = $7, hours = $8, \\
           enabled = COALESCE($9, enabled) \\
         WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(&body.name_regex)
    .bind(body.min_size)
    .bind(body.max_size)
    .bind(body.category_id)
    .bind(&body.kind)
    .bind(body.hours)
    .bind(body.enabled)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "auto_rule.update", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

/// DELETE /admin/auto-promo-rules/{id} —— 删除规则。
#[delete("/admin/auto-promo-rules/{id}")]
pub async fn auto_rules_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    sqlx::query("DELETE FROM auto_promo_rules WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "auto_rule.delete", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}
