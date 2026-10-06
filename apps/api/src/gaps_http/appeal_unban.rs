//! 申诉受理后的解封动作（0285 从 appeals.rs 拆出）。
//!
//! 拆出同时补上等级护栏：旧版只要有 `appeal.handle` 权限（class ≥ 90 即得）
//! 受理申诉，就会无条件把 `status >= 2` 的目标改回 0——包括被封的**管理员与站长**。
//! 现在要求操作者等级严格高于目标，与封禁/改等级/数值调整同一口径。

use actix_web::web;
use std::sync::Arc;

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 受理「封禁申诉」后的解封：先验等级护栏 → 恢复 status=0 → 写审计。
/// 返回是否真正恢复了账号（未恢复时调用方不应回复「账号已恢复」）。
pub(crate) async fn restore_if_outranked(
    state: &web::Data<Arc<AppState>>,
    actor_id: i64,
    actor_class: i32,
    user_id: i64,
) -> DomainResult<bool> {
    crate::admin_http::guard::ensure_outranks(
        &state.repo.db,
        actor_class,
        user_id,
    )
    .await?;
    let restored = sqlx::query(
        "UPDATE users SET status = 0 WHERE id = $1 AND status >= 2",
    )
    .bind(user_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if restored > 0 {
        state
            .repo
            .audit(Some(actor_id), "appeal.unban", Some(user_id))
            .await;
    }
    Ok(restored > 0)
}
