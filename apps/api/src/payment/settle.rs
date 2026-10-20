//! 回调入账与演示充值：验签 + 幂等动账（wallet_usd / donation_ledger / 订单状态）。

use super::{gateway_config, provider_from};
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 站内单号（幂等键）：flux-{uid}-{纳秒}（同用户重复点击各生成独立订单，回调按单号幂等）
pub fn new_order_no(user_id: i64) -> String {
    format!(
        "flux-{}-{}",
        user_id,
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    )
}

/// 实付金额形状校验（批四十四 P-173）：**两条入账入口共用的唯一一把尺**。
///
/// 原先两边都不判形状 —— 回调把网关声明的 `money` 解析完直接
/// `wallet_usd += amount`；手工补单更是由站长**手填**一个 f64，而
/// `donate_admin.rs` 的注释写着「金额以 max(实收, 0) 为准」，代码里根本没有
/// 那行 `max`。于是一次误填（负号、多打两个 0）就是凭空扣款或凭空铸币，
/// 且 `donation_ledger` 与累计捐赠（`cumulative_paid → grant_tier`）会跟着
/// 虚增，白送回馈档位。
///
/// 只判形状（有限、正数、单笔上限），不比对面额：回调侧实付低于/高于面额都
/// 可能是真实交易（部分支付、多付），当场拒收会让用户「付了钱没拿到东西」。
/// 「回调是否要求实付 ≥ 应付」另列待拍板。
pub fn check_paid_amount(amount: f64) -> DomainResult<()> {
    if !amount.is_finite() {
        return Err(DomainError::Validation("金额格式错误".into()));
    }
    if amount <= 0.0 {
        return Err(DomainError::Validation("实付金额必须为正数".into()));
    }
    if amount > 1_000_000.0 {
        return Err(DomainError::Validation("单笔实付上限 100 万美元".into()));
    }
    Ok(())
}

/// 回调统一入口：验签 + 幂等入账（wallet_usd 增加 + donation_ledger + 订单 paid）
pub async fn settle_notify(
    state: &std::sync::Arc<AppState>,
    params: &std::collections::HashMap<String, String>,
) -> DomainResult<SettleOutcome> {
    let cfg = gateway_config(state).await;
    if !cfg.available() {
        return Err(DomainError::Validation("支付通道未配置".into()));
    }
    let provider = provider_from(&cfg);
    let Some(n) = provider.verify_notify(params) else {
        return Err(DomainError::Validation("签名校验失败".into()));
    };
    if !n.status_ok {
        return Ok(SettleOutcome::NotSuccess);
    }
    // 单事务：pending → paid + 钱包入账 + 流水（重复回调第二笔起
    // rows_affected=0，天然幂等）
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let amount: f64 = n
        .amount_paid
        .parse()
        .map_err(|_| DomainError::Validation("金额格式错误".into()))?;
    // 形状不合法（0/负/超限）的回调**不入账**：订单留在 pending，人工核对
    check_paid_amount(amount)?;
    let claimed: Option<i64> = sqlx::query_scalar(
        "UPDATE payment_orders SET status='paid', trade_no=$2, amount_paid=$3, \
         paid_at=now() \
         WHERE order_no=$1 AND status='pending' RETURNING user_id",
    )
    .bind(&n.order_no)
    .bind(&n.trade_no)
    .bind(amount)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(user_id) = claimed else {
        // P-463：这里原来一律回 `Duplicate`——把「订单号查无此单」也当成
        // 「重复回调」，调用方据此给网关回 `success`。后果是钱确实在网关付了、
        // 我们这边无处入账，而**回执把重投掐了**，全站没有任何人会知道。
        // （`scripts/pt_audit_bh_donate_cheat.py` 的 H1g 只钉过管理员手工补单
        // 那条口的「单号不存在要 404」，回调这条口没人管。）
        // 现在分两种：确有此单且已 paid ⇒ 幂等成功；查无此单 ⇒ 出声并回失败，
        // 让网关按它的策略继续重投——异常要留在能被看见的地方。
        let known: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM payment_orders WHERE order_no = $1",
        )
        .bind(&n.order_no)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if known.is_none() {
            tracing::warn!(
                order = %n.order_no,
                trade = %n.trade_no,
                money = %n.amount_paid,
                "donate notify: 签名有效但查无此单，回 fail 等网关重投（需人工核对）"
            );
            return Ok(SettleOutcome::UnknownOrder);
        }
        // 已处理过的重复回调：幂等成功返回
        return Ok(SettleOutcome::Duplicate);
    };
    let balance: f64 = sqlx::query_scalar(
        "UPDATE users SET wallet_usd = wallet_usd + $2, donor = true \
         WHERE id = $1 RETURNING wallet_usd::float8",
    )
    .bind(user_id)
    .bind(amount)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO donation_ledger \
         (user_id, kind, amount_usd, balance_after, note) \
         VALUES ($1, 'topup', $2, $3, $4)",
    )
    .bind(user_id)
    .bind(amount)
    .bind(balance)
    .bind(format!("epay 回调入账 trade_no={}", n.trade_no))
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // E7 捐赠档位：同事务发放（累计含本单——本单已置 paid；CAS 防双入口）
    let cum = super::tiers::cumulative_paid(&mut tx, user_id).await?;
    let granted =
        super::tiers::grant_tier(&mut tx, state, &n.order_no, user_id, cum)
            .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 事务外通知（尽力而为，不影响回执）
    if let Some(summary) = granted {
        let email: Option<String> =
            sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_optional(&state.repo.db)
                .await
                .ok()
                .flatten();
        crate::mailer::notify_kind(
            &state.repo.db,
            user_id,
            "donate_tier",
            email,
            "捐赠回馈已发放",
            &format!("感谢支持！本次捐赠触发回馈档位 {summary}，已自动入账。"),
        )
        .await;
        crate::ops_webhook::broadcast_ops_spawn(
            state,
            format!("收到捐赠 ${amount:.2}（{summary}）"),
        );
    }
    return Ok(SettleOutcome::Paid);
}

#[derive(Debug, PartialEq)]
pub enum SettleOutcome {
    Paid,
    Duplicate,
    NotSuccess,
    /// 签名有效但 `payment_orders` 里查无此单（P-463）：调用方必须回失败，
    /// 不能像 Duplicate 那样 ack success 把网关的重投掐掉。
    UnknownOrder,
}

/// 演示环境模拟充值（FLUX_DEMO=1 专用，U4 从 topup 主路径移出）：
/// 直接加余额 + 流水标注「模拟」——真实站点永不走此路径（gateway 未配置时
/// 且未开 FLUX_DEMO 一律拒单）。
pub async fn demo_topup(
    state: &AppState,
    user_id: i64,
    amount_usd: f64,
    channel: &str,
) -> DomainResult<serde_json::Value> {
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let balance: f64 = sqlx::query_scalar(
        "UPDATE users SET wallet_usd = wallet_usd + $2, donor = true \
         WHERE id = $1 RETURNING wallet_usd::float8",
    )
    .bind(user_id)
    .bind(amount_usd)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO donation_ledger \
         (user_id, kind, amount_usd, balance_after, note) \
         VALUES ($1, 'topup', $2, $3, $4)",
    )
    .bind(user_id)
    .bind(amount_usd)
    .bind(balance)
    .bind(format!("模拟支付成功（{channel}）"))
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(user_id), "donate_topup_demo", None)
        .await;
    Ok(serde_json::json!({ "wallet_usd": balance }))
}

/// 金额形状尺的单测（0/负/无穷/上限四支）。
/// 注：这条尺真正接的是两个入口（回调与手工补单），活体红线见
/// `scripts/pt_audit_bh_donate_cheat.py` 的 H1 组 —— 本单测只钉形状口径，
/// 不能证明它被接进了链路。
#[cfg(test)]
mod amount_tests {
    use super::check_paid_amount;

    #[test]
    fn non_positive_and_non_finite_are_rejected() {
        for bad in [-12.5_f64, 0.0, -0.0] {
            assert!(check_paid_amount(bad).is_err(), "{bad} 必须拒");
        }
        assert!(check_paid_amount(f64::INFINITY).is_err());
        assert!(check_paid_amount(f64::NAN).is_err());
    }

    #[test]
    fn ceiling_is_inclusive_at_one_million() {
        assert!(check_paid_amount(5.0).is_ok());
        assert!(check_paid_amount(1_000_000.0).is_ok());
        assert!(check_paid_amount(1_000_000.01).is_err());
    }
}
