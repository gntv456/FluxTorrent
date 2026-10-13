//! 用户级权限覆盖（批五十五按域拆出，原在 `perms.rs`）：某人的权限全貌 +
//! 授予/拒绝/清除覆盖。角色权限矩阵那半仍在 `perms.rs`。
//!
//! 拆件不只是为行数门禁：本件三条写口都要「审计带对象」（动了哪条权限、
//! 授予还是清除），与矩阵批量的「整批校验 + 同事务」是两套判据，混在一个
//! 文件里正是上一批 worker 告警那族的形状——同一把尺写到第二遍就漏。

use actix_web::{get, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::{ensure_outranks, staff};

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
            .map(|(k, g)| {
                serde_json::json!({ "permission_key": k,
                "granted": g })
            })
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
        .audit_detail(
            Some(auth.id),
            "user.permission_set",
            Some(body.user_id),
            body.note.as_deref(),
            Some(serde_json::json!({
                "permission_key": body.permission_key,
                "granted": body.granted,
            })),
        )
        .await;
    Ok(ok(serde_json::json!({
        "user_id": body.user_id,
        "permission_key": body.permission_key,
        "granted": body.granted,
    })))
}
