//! P2-6b 管理员改名（写入 username_change_logs）
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::{modify_log, staff};

// ============ P2-6b 管理员改名（写入 username_change_logs） ============

#[derive(Deserialize)]
struct RenameReq {
    new_name: String,
}

#[post("/admin/users/{id}/rename")]
async fn admin_user_rename(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<RenameReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::USER_RESETPASS,
    )
    .await?;
    let uid = path.into_inner();
    // 等级护栏（审计修复）：改名语义同重置密码，操作者须严格高于目标用户
    {
        let target_class: Option<i32> =
            sqlx::query_scalar("SELECT class_id FROM users WHERE id = $1")
                .bind(uid)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        let tc = target_class.ok_or(DomainError::NotFound(uid))?;
        if auth.class_id <= tc {
            return Err(DomainError::Forbidden);
        }
    }
    let new_name = body.new_name.trim();
    if new_name.is_empty() || new_name.len() > 32 {
        return Err(DomainError::Validation("用户名长度 1-32".into()));
    }
    let db = &state.repo.db;
    let old: Option<String> =
        sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
            .bind(uid)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(old_name) = old else {
        return Err(DomainError::NotFound(uid));
    };
    if old_name == new_name {
        return Ok(ok(serde_json::json!({ "ok": true })));
    }
    let taken: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE username = $1 AND id <> $2)",
    )
    .bind(new_name)
    .bind(uid)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if taken {
        return Err(DomainError::Validation("用户名已被占用".into()));
    }
    sqlx::query("UPDATE users SET username = $2 WHERE id = $1")
        .bind(uid)
        .bind(new_name)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO username_change_logs (uid, old_name, new_name, operator) VALUES ($1, $2, $3, $4)",
    )
    .bind(uid)
    .bind(&old_name)
    .bind(new_name)
    .bind(auth.id)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "user.rename", Some(uid))
        .await;
    modify_log(
        db,
        uid,
        Some(auth.id),
        &format!("改名 {old_name} → {new_name}"),
    )
    .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}
