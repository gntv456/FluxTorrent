//! 赠礼类购买的接收人落实（0421，自 shop.rs 拆出守 300 行门禁）。
//!
//! 背景：货架文案承诺 gift_spark「赠予其他用户，可指定份数」，旧实现
//! 却既无接收人参数、效果又 earn_spark 记买家本人——花 1200 只回账
//! 1000、收礼人什么都收不到的亏本陷阱。本模块把「赠送给谁」落成
//! user id，全部校验前置于扣款。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 把赠礼接收人用户名落成 user id。空 / 不存在 / 已注销或封禁 / 就是
/// 买家自己，一律 Validation（400）——动账之前拦干净。
pub async fn resolve_gift_recipient(
    db: &PgPool,
    buyer_id: i64,
    to: Option<&str>,
) -> DomainResult<i64> {
    let to = to.map(str::trim).unwrap_or("");
    if to.is_empty() {
        return Err(DomainError::Validation(
            "赠送魔力需要指定接收人用户名".into(),
        ));
    }
    let target: Option<(i64, i16)> =
        sqlx::query_as("SELECT id, status FROM users WHERE username = $1")
            .bind(to)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((tid, tstatus)) = target else {
        return Err(DomainError::Validation(format!("接收人不存在：{to}")));
    };
    if tstatus >= 2 {
        return Err(DomainError::Validation(format!(
            "接收人已注销或封禁：{to}"
        )));
    }
    if tid == buyer_id {
        return Err(DomainError::Validation(
            "不能赠送给自己——赠礼必须指定其他用户".into(),
        ));
    }
    Ok(tid)
}
