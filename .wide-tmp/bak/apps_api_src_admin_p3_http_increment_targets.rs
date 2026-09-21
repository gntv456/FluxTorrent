//! increment-bulk 校验与目标集合选择（从 increment_bulk.rs 拆出）。

use actix_web::web;
use std::sync::Arc;

use super::IncrementBulkReq;

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 入参校验（上限口径沿用旧端点：火花 ±100 万；上传量 ±10TB（GB 换算）；邀请/补签卡 1-50）
pub(super) async fn increment_bulk_validate(
    state: &web::Data<Arc<AppState>>,
    auth: &crate::http::AuthUser,
    body: &IncrementBulkReq,
) -> DomainResult<()> {
    let perm = match body.kind.as_str() {
        "spark" => crate::authz::perm::USER_AMOUNTBONUS,
        "uploaded" | "invite" | "resub_card" => {
            crate::authz::perm::USER_AMOUNTUPLOAD
        }
        _ => {
            return Err(DomainError::Validation(
                "kind 取值 spark/uploaded/invite/resub_card".into(),
            ))
        }
    };
    crate::authz::require_perm(state, auth, perm).await?;
    if body.amount == 0 {
        return Err(DomainError::Validation("数量不能为 0".into()));
    }
    // 上限口径沿用旧端点：火花 ±100 万；上传量 ±10TB（GB 换算）；邀请/补签卡 1-50
    match body.kind.as_str() {
        "spark" if body.amount.abs() > 1_000_000 => {
            return Err(DomainError::Validation("魔力单次 ±1,000,000".into()));
        }
        "uploaded" if body.amount.abs() > 10 * 1024 => {
            return Err(DomainError::Validation(
                "上传量单次 ±10TB（GB）".into(),
            ));
        }
        "invite" if body.amount.abs() > 50 => {
            return Err(DomainError::Validation("邀请单次 ±50".into()));
        }
        "invite" if body.days.is_some_and(|d| !(1..=365).contains(&d)) => {
            return Err(DomainError::Validation(
                "临时邀请有效期 1-365 天".into(),
            ));
        }
        "invite" if body.days.is_some() && body.amount <= 0 => {
            return Err(DomainError::Validation("临时邀请数量需为正数".into()));
        }
        "resub_card" if !(1..=50).contains(&body.amount) => {
            return Err(DomainError::Validation("补签卡单次 1-50".into()));
        }
        _ => {}
    }
    if body.user_ids.is_empty()
        && body.classes.is_empty()
        && body.roles.is_empty()
    {
        return Err(DomainError::Validation(
            "需选择目标：等级 / 职务 / 指定用户".into(),
        ));
    }
    if body.user_ids.len() > 500 {
        return Err(DomainError::Validation("指定用户单批最多 500".into()));
    }
    Ok(())
}

/// 目标集合选择：指定用户优先（不叠加等级筛选，口径同 NP：receiver 直发）；
/// 否则 classes OR roles，未封禁（status<2）
pub(super) async fn increment_bulk_targets(
    db: &sqlx::PgPool,
    body: &IncrementBulkReq,
) -> DomainResult<Vec<i64>> {
    let targets: Vec<i64> = if !body.user_ids.is_empty() {
        sqlx::query_scalar(
            "SELECT id FROM users WHERE id = ANY($1) AND status < 2",
        )
        .bind(&body.user_ids)
        .fetch_all(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    } else {
        let mut clauses: Vec<String> = Vec::new();
        if !body.classes.is_empty() {
            clauses.push("class_id = ANY($1)".into());
        }
        if !body.roles.is_empty() {
            clauses.push(
                "id IN (SELECT user_id FROM user_roles WHERE role_key = ANY($2))"
                    .into(),
            );
        }
        let sql = format!(
            "SELECT id FROM users WHERE status < 2 AND ({}) ORDER BY id",
            clauses.join(" OR ")
        );
        sqlx::query_scalar(&sql)
            .bind(&body.classes)
            .bind(&body.roles)
            .fetch_all(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
    };
    if targets.is_empty() {
        return Err(DomainError::Validation("没有符合条件的目标用户".into()));
    }
    Ok(targets)
}
