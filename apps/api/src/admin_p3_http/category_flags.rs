//! 分类归属模式 + 自动过审开关（从 section_public.rs 拆出守行数门，
//! 加速批五期 2026-11-26；本口只写 categories 表两列，不碰字典内容，
//! 故不挂 section-dict 缓存失效）。

use actix_web::{put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

/// 缺省 = 不改归属；`0` = 显式清除归属（回到「全模式可用」）；其余 = 改归属。
/// 早先实现是裸 `SET mode_id = $2`，前端只勾「自动过审」不带 mode_id 时会把
/// 归属写成 NULL（等于静默改分类归属）。
#[derive(Deserialize)]
struct CategoryFlagsReq {
    #[serde(default)]
    mode_id: Option<i32>,
    #[serde(default)]
    auto_approve: Option<bool>,
}

#[put("/admin/categories/{id}/flags")]
pub(super) async fn category_flags(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<CategoryFlagsReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE categories SET \
           mode_id = CASE WHEN $2::int IS NULL THEN mode_id \
                          WHEN $2::int = 0 THEN NULL \
                          ELSE $2::int END, \
           auto_approve = COALESCE($3, auto_approve) \
         WHERE id = $1",
    )
    .bind(id)
    .bind(body.mode_id)
    .bind(body.auto_approve)
    .execute(&state.repo.db)
    .await
    .map_err(|e| crate::errors::db_to_domain(e, "分类归属模式"))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "category.flags", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}
