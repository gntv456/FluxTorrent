//! 管理端鉴权守卫：staff 门槛 + 等级护栏。

use actix_web::{web, HttpRequest};

use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

pub(super) async fn staff(
    req: &HttpRequest,
    state: &web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<crate::http::AuthUser> {
    let auth = require_auth(req, state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_PANEL)
        .await?;
    Ok(auth)
}

/// 权限护栏（NexusPHP checkPermission 口径）：操作者等级须**严格大于**目标用户。
///
/// 用于所有会修改目标用户的端点。此前 status / flags / adjust 仅有 staff(>=90) 守卫，
/// 导致总版主(93) 可封禁、挂起、扣减管理员(94) / 主管(95) / 站长(99) 的数据。
pub(super) async fn ensure_outranks(
    db: &sqlx::PgPool,
    actor_class: i32,
    target_user_id: i64,
) -> DomainResult<()> {
    let target_class: Option<i32> =
        sqlx::query_scalar("SELECT class_id FROM users WHERE id = $1")
            .bind(target_user_id)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let tc = target_class.ok_or(DomainError::NotFound(target_user_id))?;
    if actor_class <= tc {
        return Err(DomainError::Forbidden);
    }
    Ok(())
}
