//! 共享助手（staff 门卫 / 用户修改记录 / 批量 ID 校验）
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{web, HttpRequest};

use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

pub(crate) const MAX_BATCH: usize = 500;

pub(crate) async fn staff(
    req: &HttpRequest,
    state: &web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<crate::http::AuthUser> {
    let auth = require_auth(req, state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_PANEL)
        .await?;
    Ok(auth)
}

/// 用户修改记录（参考站 UserModifyLog 口径）：管理动作按用户落一条可读摘要
pub async fn modify_log(
    db: &sqlx::PgPool,
    uid: i64,
    modifier: Option<i64>,
    content: &str,
) {
    let _ = sqlx::query(
        "INSERT INTO user_modify_logs (uid, modifier, \
         content) VALUES ($1, $2, $3)",
    )
    .bind(uid)
    .bind(modifier)
    .bind(content)
    .execute(db)
    .await;
}

pub(crate) fn check_ids(ids: &[i64]) -> DomainResult<()> {
    if ids.is_empty() {
        return Err(DomainError::Validation("缺少目标 ID".into()));
    }
    if ids.len() > MAX_BATCH {
        return Err(DomainError::Validation(format!(
            "单批最多 {MAX_BATCH} 条"
        )));
    }
    Ok(())
}
