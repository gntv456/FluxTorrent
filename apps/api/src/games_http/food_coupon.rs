//! 口粮券核销助手：把「做种行为联动资源」的消耗并进玩法事务。
//!
//! 与 `linkage.rs::use_coupon` 同一安全模型（`FOR UPDATE` 锁用户行 +
//! `coupon_uses` 幂等键防双花），但这里是**事务内版本** —— 宠物投喂要把
//! 「扣券 / 扣款 + 喂食」并成同一笔，分两次写就是「券扣了、宠物没喂到」。
//! 锁序与 linkage 一致：先 `users` 行锁，再查 `coupon_uses`。

use sqlx::Transaction;

use crate::economy_http::{spend_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};

use super::pool::dberr;

/// 一次核销的四种结局：`Fresh` = 首次成功扣券；`Replayed` = 同键已扣过
/// （本次不重复扣）；`Insufficient` = 没券可扣。
pub(super) enum CouponOutcome {
    Fresh,
    Replayed,
    Insufficient,
}

/// 核销 1 张口粮券（事务内）。重放不重复扣，余额不足不动账。
pub(super) async fn consume_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    idem: &str,
) -> DomainResult<CouponOutcome> {
    sqlx::query("SELECT food_coupons FROM users WHERE id = $1 FOR UPDATE")
        .bind(uid)
        .execute(&mut **tx)
        .await
        .map_err(dberr)?;
    let seen: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM coupon_uses WHERE idem = $1)",
    )
    .bind(idem)
    .fetch_one(&mut **tx)
    .await
    .map_err(dberr)?;
    if seen {
        return Ok(CouponOutcome::Replayed);
    }
    let bal: i64 = sqlx::query_scalar(
        "SELECT COALESCE(food_coupons, 0)::bigint FROM users WHERE id = $1",
    )
    .bind(uid)
    .fetch_one(&mut **tx)
    .await
    .map_err(dberr)?;
    if bal < 1 {
        return Ok(CouponOutcome::Insufficient);
    }
    sqlx::query("INSERT INTO coupon_uses (idem, user_id) VALUES ($1, $2)")
        .bind(idem)
        .bind(uid)
        .execute(&mut **tx)
        .await
        .map_err(dberr)?;
    sqlx::query(
        "UPDATE users SET food_coupons = food_coupons - 1 WHERE id = $1",
    )
    .bind(uid)
    .execute(&mut **tx)
    .await
    .map_err(dberr)?;
    Ok(CouponOutcome::Fresh)
}

/// 领取侧撞重放的处置：报「已受理」并让整笔事务回滚（P-640）。
///
/// 语义与 [`pay_feed_tx`] 对投喂重放完全一致，但**不复用**
/// `economy_http::expect_spent`——那句文案写给批量发放的操作者
/// （「请检查目标名单里是否有重复用户」），会员个人动作拿到它无从行动。
/// 旧写法是发放撞重放时继续报 200 并把 `pending` 清零，两个后果叠在一起：
/// 「接口成功而用户零到账」（`spend.rs:219` 成文口径点名的最贵形状），
/// 且把这枚键之后**新攒**的产出一起抹掉。
pub(super) fn expect_fresh(out: SpendOutcome) -> DomainResult<()> {
    match out {
        SpendOutcome::Spent => Ok(()),
        SpendOutcome::Replayed => Err(DomainError::Validation(
            "这次领取已受理，请勿重复提交".into(),
        )),
    }
}

/// 投喂扣款：走魔力（原路径）或用 1 张口粮券抵扣（行为联动出口）。
/// 返回 `(实扣魔力, 是否用了券)`。
pub(super) async fn pay_feed_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    idem: &str,
    use_coupon: bool,
    cost: i64,
) -> DomainResult<(i64, bool)> {
    if !use_coupon {
        if !matches!(
            spend_spark_tx(tx, uid, cost, "game", idem, "pet_feed", 0).await?,
            SpendOutcome::Spent
        ) {
            return Err(DomainError::Validation(
                "这次投喂已受理，请勿重复提交".into(),
            ));
        }
        return Ok((cost, false));
    }
    match consume_tx(tx, uid, &format!("{idem}:c")).await? {
        CouponOutcome::Fresh => Ok((0, true)),
        CouponOutcome::Replayed => Err(DomainError::Validation(
            "这次投喂已受理，请勿重复提交".into(),
        )),
        CouponOutcome::Insufficient => Err(DomainError::Validation(
            "口粮券不足：每日做种满 6 小时可得 1 张".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 投喂与领取对「同一枚键第二次到达」必须同一口径：都是拒，不是
    /// 「成功但不付」。负控翻 `expect_fresh` 的 `Replayed` 分支即红两条。
    #[test]
    fn replay_is_rejected_not_silently_accepted() {
        assert!(expect_fresh(SpendOutcome::Spent).is_ok());
        let e = expect_fresh(SpendOutcome::Replayed).unwrap_err();
        let msg = format!("{:?}", e);
        assert!(msg.contains("已受理"), "重放文案不像重放：{msg}");
        // 批量发放那句「请检查目标名单里是否有重复用户」不许搬给会员
        assert!(!msg.contains("重复用户"), "文案是发放名单口径：{msg}");
    }
}
