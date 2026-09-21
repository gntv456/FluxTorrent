use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::{ensure_outranks, staff};

// ============ 职务管理（user_roles，可兼任） ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct RoleRow {
    key: String,
    name: String,
    descr: Option<String>,
}

/// 职务字典（管理界面渲染用）
#[get("/admin/roles")]
async fn role_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<RoleRow> =
        sqlx::query_as("SELECT key, name, descr FROM roles ORDER BY sort, key")
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct UserRoleRow {
    user_id: i64,
    role_key: String,
    granted_by: Option<i64>,
    granted_at: chrono::DateTime<chrono::Utc>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Deserialize)]
struct UserRoleQ {
    #[serde(default)]
    user_id: Option<i64>,
}

/// 用户持有的职务（指定 user_id 则只看该用户）
#[get("/admin/user-roles")]
async fn user_role_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<UserRoleQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<UserRoleRow> =
        match q.user_id {
            Some(uid) => sqlx::query_as(
                "SELECT user_id, role_key, granted_by, granted_at, expires_at \
             FROM user_roles WHERE user_id = $1 ORDER BY granted_at",
            )
            .bind(uid)
            .fetch_all(&state.repo.db)
            .await,
            None => sqlx::query_as(
                "SELECT user_id, role_key, granted_by, granted_at, expires_at \
             FROM user_roles ORDER BY granted_at DESC LIMIT 200",
            )
            .fetch_all(&state.repo.db)
            .await,
        }
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct GrantRoleReq {
    user_id: i64,
    role_key: String,
    /// RFC3339；缺省为永久
    #[serde(default)]
    expires_at: Option<String>,
}

/// 授予职务：须严格高于目标等级（与其它用户操作同一护栏）
#[post("/admin/user-roles")]
async fn user_role_grant(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<GrantRoleReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    ensure_outranks(&state.repo.db, auth.class_id, body.user_id).await?;
    let exists: Option<String> =
        sqlx::query_scalar("SELECT key FROM roles WHERE key = $1")
            .bind(&body.role_key)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if exists.is_none() {
        return Err(DomainError::Validation("职务不存在".into()));
    }
    let expires = match &body.expires_at {
        Some(t) if !t.trim().is_empty() => Some(
            chrono::DateTime::parse_from_rfc3339(t)
                .map_err(|_| {
                    DomainError::Validation("到期时间格式无效".into())
                })?
                .with_timezone(&chrono::Utc),
        ),
        _ => None,
    };
    crate::authz::grant_role(
        &state.repo.db,
        body.user_id,
        &body.role_key,
        auth.id,
        expires,
    )
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "user.role_grant", Some(body.user_id))
        .await;
    Ok(ok(
        serde_json::json!({ "user_id": body.user_id, "role_key": body.role_key }),
    ))
}

/// 撤销职务
#[delete("/admin/user-roles/{user_id}/{role_key}")]
async fn user_role_revoke(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, String)>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let (uid, role_key) = path.into_inner();
    ensure_outranks(&state.repo.db, auth.class_id, uid).await?;
    let n = crate::authz::revoke_role(&state.repo.db, uid, &role_key)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if n == 0 {
        return Err(DomainError::NotFound(uid));
    }
    state
        .repo
        .audit(Some(auth.id), "user.role_revoke", Some(uid))
        .await;
    Ok(ok(serde_json::json!({ "deleted": n })))
}
