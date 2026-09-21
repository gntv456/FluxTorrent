use actix_web::{get, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::{ensure_outranks, staff};

// ============ 权限配置（角色权限矩阵 + 用户级权限分配） ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct PermRow {
    key: String,
    name: String,
    category: String,
    descr: Option<String>,
    /// 是否有真实生效点（false = 界面应标注「未接入」）
    implemented: bool,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct PermRoleRow {
    role_type: String,
    role_key: String,
    name: String,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct PermGrantRow {
    role_type: String,
    role_key: String,
    permission_key: String,
}

/// 权限矩阵：权限清单 + 可配角色 + 当前授权映射
#[get("/admin/permission-matrix")]
async fn permission_matrix(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let permissions: Vec<PermRow> = sqlx::query_as(
        "SELECT key, name, category, descr, implemented FROM permissions \
             ORDER BY category, sort, key",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 可配角色 = 已有配置的等级档 + 全部职务
    let roles: Vec<PermRoleRow> = sqlx::query_as(
        "SELECT t.role_type, t.role_key, COALESCE(uc.name, r.name, t.role_key) AS name
         FROM (
             SELECT DISTINCT 'class'::text AS role_type, role_key, 0 AS ord
             FROM role_permissions WHERE role_type = 'class'
             UNION ALL
             SELECT 'role', key, sort FROM roles
         ) t
         LEFT JOIN user_classes uc
                ON t.role_type = 'class'
               AND uc.id = CASE WHEN t.role_key ~ '^[0-9]+$'
                                THEN t.role_key::integer
                                ELSE -1 END
         LEFT JOIN roles r ON t.role_type = 'role' AND r.key = t.role_key
         ORDER BY t.role_type DESC, t.ord, t.role_key",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let grants: Vec<PermGrantRow> = sqlx::query_as(
        "SELECT role_type, role_key, permission_key FROM role_permissions WHERE granted",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "permissions": permissions,
        "roles": roles,
        "grants": grants,
    })))
}

#[derive(Deserialize)]
struct PermMatrixItem {
    role_type: String,
    role_key: String,
    permission_key: String,
    granted: bool,
}

#[derive(Deserialize)]
struct PermMatrixReq {
    items: Vec<PermMatrixItem>,
}

/// 批量更新角色权限（勾选/取消）。仅站长可改，避免越权提权。
#[put("/admin/permission-matrix")]
async fn permission_matrix_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PermMatrixReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    if body.items.len() > 500 {
        return Err(DomainError::Validation("单次最多 500 项".into()));
    }
    let mut changed = 0u64;
    for it in &body.items {
        if !["class", "role"].contains(&it.role_type.as_str()) {
            return Err(DomainError::Validation(
                "role_type 取值 class/role".into(),
            ));
        }
        let exists: Option<String> =
            sqlx::query_scalar("SELECT key FROM permissions WHERE key = $1")
                .bind(&it.permission_key)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        if exists.is_none() {
            return Err(DomainError::Validation(format!(
                "权限不存在：{}",
                it.permission_key
            )));
        }
        let n = if it.granted {
            sqlx::query(
                "INSERT INTO role_permissions (role_type, role_key, permission_key, granted)
                 VALUES ($1, $2, $3, true) ON CONFLICT DO NOTHING",
            )
            .bind(&it.role_type)
            .bind(&it.role_key)
            .bind(&it.permission_key)
            .execute(&state.repo.db)
            .await
        } else {
            sqlx::query(
                "DELETE FROM role_permissions
                 WHERE role_type = $1 AND role_key = $2 AND permission_key = $3",
            )
            .bind(&it.role_type)
            .bind(&it.role_key)
            .bind(&it.permission_key)
            .execute(&state.repo.db)
            .await
        }
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
        changed += n;
    }
    state
        .repo
        .audit(Some(auth.id), "permission_matrix.update", None)
        .await;
    Ok(ok(serde_json::json!({ "changed": changed })))
}

#[derive(Deserialize)]
struct UserPermQ {
    user_id: i64,
}

/// 某用户的权限全貌：角色所得（effective）+ 用户级覆盖（overrides）
#[get("/admin/user-permissions")]
async fn user_permission_view(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<UserPermQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let class_id: Option<i32> =
        sqlx::query_scalar("SELECT class_id FROM users WHERE id = $1")
            .bind(q.user_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(class_id) = class_id else {
        return Err(DomainError::NotFound(q.user_id));
    };
    let effective =
        crate::authz::user_perm_keys(&state.repo.db, class_id, q.user_id).await;
    let overrides =
        crate::authz::user_permission_overrides(&state.repo.db, q.user_id)
            .await;
    let roles = crate::authz::user_role_keys(&state.repo.db, q.user_id).await;
    Ok(ok(serde_json::json!({
        "user_id": q.user_id,
        "class_id": class_id,
        "roles": roles,
        "effective": effective,
        "overrides": overrides.into_iter()
            .map(|(k, g)| serde_json::json!({ "permission_key": k, "granted": g }))
            .collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct SetUserPermReq {
    user_id: i64,
    permission_key: String,
    /// true=额外授予 / false=显式拒绝 / null=清除覆盖（继承角色）
    #[serde(default)]
    granted: Option<bool>,
    #[serde(default)]
    note: Option<String>,
}

/// 设置用户级权限覆盖（授予 / 拒绝 / 清除）
#[put("/admin/user-permissions")]
async fn user_permission_set(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SetUserPermReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_CLASS)
        .await?;
    ensure_outranks(&state.repo.db, auth.class_id, body.user_id).await?;
    let exists: Option<String> =
        sqlx::query_scalar("SELECT key FROM permissions WHERE key = $1")
            .bind(&body.permission_key)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if exists.is_none() {
        return Err(DomainError::Validation("权限不存在".into()));
    }
    crate::authz::set_user_permission(
        &state.repo.db,
        body.user_id,
        &body.permission_key,
        body.granted,
        auth.id,
        body.note.as_deref(),
    )
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "user.permission_set", Some(body.user_id))
        .await;
    Ok(ok(serde_json::json!({
        "user_id": body.user_id,
        "permission_key": body.permission_key,
        "granted": body.granted,
    })))
}
