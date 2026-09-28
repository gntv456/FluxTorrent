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
    // 单事务：pending → paid + 钱包入账 + 流水（重复回调第二笔起 rows_affected=0，天然幂等）
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
