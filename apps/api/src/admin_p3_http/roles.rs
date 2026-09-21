//! 职务字典 CRUD（0064：roles.manage 仅站长）
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

// ============ 职务字典 CRUD（0064：roles.manage 仅站长） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct RoleDefRow {
    key: String,
    name: String,
    descr: Option<String>,
    sort: i32,
}

#[get("/admin/roles-dict")]
async fn roles_dict_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<RoleDefRow> = sqlx::query_as(
        "SELECT key, name, descr, sort FROM roles ORDER BY sort, key",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
struct RoleDefReq {
    key: String,
    name: String,
    #[serde(default)]
    descr: Option<String>,
    #[serde(default)]
    sort: Option<i32>,
}

#[post("/admin/roles")]
async fn roles_dict_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RoleDefReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ROLES_MANAGE)
        .await?;
    let key = body.key.trim();
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(DomainError::Validation(
            "key 仅允许小写字母/数字/下划线".into(),
        ));
    }
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("名称必填".into()));
    }
    // 预查重：roles.key 是主键，裸 INSERT 撞键会以 500 泄漏；
    // rows_affected 判重只对 ON CONFLICT DO NOTHING 生效，普通 INSERT 冲突必然报错
    let dup: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM roles WHERE key = $1)")
            .bind(&key)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
    if dup {
        return Err(DomainError::Validation("职务已存在".into()));
    }
    sqlx::query(
        "INSERT INTO roles (key, name, descr, sort) VALUES ($1, \
     $2, $3, COALESCE($4, 0))",
    )
    .bind(key)
    .bind(body.name.trim())
    .bind(body.descr.clone())
    .bind(body.sort)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "roles.add", None).await;
    Ok(ok(serde_json::json!({ "key": key })))
}

#[put("/admin/roles/{key}")]
async fn roles_dict_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<RoleDefReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ROLES_MANAGE)
        .await?;
    let key = path.into_inner();
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("名称必填".into()));
    }
    // key 不可改（user_roles/role_permissions 外键引用它）；改名连带刷新权限缓存
    let n = sqlx::query(
        "UPDATE roles SET name = $2, descr = $3, \
         sort = COALESCE($4, sort) WHERE key = $1",
    )
    .bind(&key)
    .bind(body.name.trim())
    .bind(body.descr.clone())
    .bind(body.sort)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(0));
    }
    crate::http::bump_guard_ver(&state).await;
    state.repo.audit(Some(auth.id), "roles.update", None).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/roles/{key}")]
async fn roles_dict_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ROLES_MANAGE)
        .await?;
    let key = path.into_inner();
    let holders: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM user_roles WHERE role_key = $1",
    )
    .bind(&key)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if holders > 0 {
        return Err(DomainError::Validation(format!(
            "仍有 {holders} 人持有该职务，请先撤销全部授予"
        )));
    }
    let n = sqlx::query("DELETE FROM roles WHERE key = $1")
        .bind(&key)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(0));
    }
    crate::http::bump_guard_ver(&state).await;
    state.repo.audit(Some(auth.id), "roles.del", None).await;
    Ok(ok(serde_json::json!({ "deleted": key })))
}
