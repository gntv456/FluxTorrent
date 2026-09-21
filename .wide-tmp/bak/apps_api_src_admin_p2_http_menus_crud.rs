//! 菜单项 CRUD（第五轮 P2）：新增/更新/删除。
//! 从 admin_p2_http/menus.rs 按域拆出。

use actix_web::{delete, post, put, web, HttpRequest, HttpResponse};

use super::staff;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::menus::{menu_item_validate, MenuItemReq};

#[post("/admin/menu-items")]
pub async fn menu_items_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MenuItemReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // 审计修复：站点级配置写操作须 SETTINGS_MANAGE（此前仅 staff() 90 档即可改）
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let label = body.label.as_deref().unwrap_or("").trim().to_string();
    let url = body.url.as_deref().unwrap_or("").trim().to_string();
    let location = body.location.as_deref().unwrap_or("sidebar");
    if label.is_empty() || label.len() > 50 || url.is_empty() || url.len() > 300
    {
        return Err(DomainError::Validation("名称 1-50，链接 1-300".into()));
    }
    if !["sidebar", "footer", "topbar"].contains(&location) {
        return Err(DomainError::Validation(
            "location 取值 sidebar/footer/topbar".into(),
        ));
    }
    menu_item_validate(&state.repo.db, &body, None).await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO menu_items (location, label, url, parent_id, target, min_class, sort, enabled) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
    )
    .bind(location)
    .bind(label)
    .bind(url)
    .bind(body.parent_id.unwrap_or(0))
    .bind(body.target.as_deref().unwrap_or("_self"))
    .bind(body.min_class.unwrap_or(0))
    .bind(body.sort.unwrap_or(0))
    .bind(body.enabled.unwrap_or(true))
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "menu_item.add", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/menu-items/{id}")]
pub async fn menu_items_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<MenuItemReq>,
) -> DomainResult<HttpResponse> {
    let path_id = path.into_inner();
    let auth = staff(&req, &state).await?;
    // 审计修复：站点级配置写操作须 SETTINGS_MANAGE（此前仅 staff() 90 档即可改）
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    menu_item_validate(&state.repo.db, &body, Some(path_id)).await?;
    let n = sqlx::query(
        "UPDATE menu_items SET \
            location = COALESCE($2, location), label = COALESCE($3, label), url = COALESCE($4, url), \
            parent_id = COALESCE($5, parent_id), target = COALESCE($6, target), \
            min_class = COALESCE($7, min_class), sort = COALESCE($8, sort), enabled = COALESCE($9, enabled) \
         WHERE id = $1",
    )
    .bind(path_id)
    .bind(body.location.clone())
    .bind(body.label.clone())
    .bind(body.url.clone())
    .bind(body.parent_id)
    .bind(body.target.clone())
    .bind(body.min_class)
    .bind(body.sort)
    .bind(body.enabled)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(path_id));
    }
    state
        .repo
        .audit(Some(auth.id), "menu_item.update", None)
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/menu-items/{id}")]
pub async fn menu_items_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let path_id = path.into_inner();
    let auth = staff(&req, &state).await?;
    // 审计修复：站点级配置写操作须 SETTINGS_MANAGE（此前仅 staff() 90 档即可改）
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let children: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM menu_items WHERE parent_id = $1",
    )
    .bind(path_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if children > 0 {
        return Err(DomainError::Validation(
            "存在子菜单，先删除或移动子菜单".into(),
        ));
    }
    let n = sqlx::query("DELETE FROM menu_items WHERE id = $1")
        .bind(path_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(path_id));
    }
    state
        .repo
        .audit(Some(auth.id), "menu_item.delete", None)
        .await;
    Ok(ok(serde_json::json!({ "deleted": n })))
}
