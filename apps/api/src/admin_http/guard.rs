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
pub(crate) async fn ensure_outranks(
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

/// 改档口的字典校验：`users.class_id` 有 FK 指向 `user_classes`，而档位号
/// 是**不连续**的（0..12、20、91..95、98、99 —— 没有 90/13..19/96/97）。
/// 批四十三实测 `class_id=90` 直接撞外键 ⇒ 后台报 500「内部错误」，操作者
/// 以为面板坏了；单人改档与批量改档两处同病，判据收在这一处。
pub(crate) async fn ensure_class_exists(
    db: &sqlx::PgPool,
    class_id: i32,
) -> DomainResult<()> {
    let known: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_classes WHERE id = $1)",
    )
    .bind(class_id)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !known {
        return Err(DomainError::Validation(format!(
            "等级 {class_id} 不在等级字典里（改档只能用 user_classes 里已有的档位）"
        )));
    }
    Ok(())
}
