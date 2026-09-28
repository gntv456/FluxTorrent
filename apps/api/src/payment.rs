//! 支付网关抽象（U4 §12.1）：Provider trait + 易支付协议族实现。
//!
//! 设计纪律：
//! - 网关凭证全部在 site_settings（payment_*，密钥 secret 型后台回掩码），代码不绑商户；
//! - 未配置（provider=none / 凭证缺失）= 通道未开通：topup 拒单、前端显示「通道未开通」；
//! - 回调验签 + order_no 幂等（payment_orders 唯一键 + donation_ledger 复用入账）；
//! - 仅实现「跳转支付」（submit.php?...）形态：最通用、无异步通知服务器要求，
//!   同步回跳由前端轮询订单状态；异步 notify 端点同样实现（get 请求 + MD5 验签）。

mod epay;
mod settle;
#[cfg(test)]
mod tests;
mod tiers;

// 重导出保留原公开面：epay_sign/SettleOutcome 等仅单测与内部使用，
// bin crate 的 pub use 会触发 unused_imports，按原 pub fn 语义显式放行。
#[allow(unused_imports)]
pub use epay::{epay_sign, EpayProvider};
#[allow(unused_imports)]
pub use settle::{demo_topup, new_order_no, settle_notify, SettleOutcome};
pub use tiers::{cumulative_paid, grant_tier, load_tiers};

use crate::state::AppState;

/// 网关配置快照（每次充值读取，密钥不留进程内缓存——低频路径，安全优先）
#[derive(Clone)]
pub struct GatewayConfig {
    pub provider: String, // none / epay
    pub gateway_url: String,
    pub pid: String,
    pub key: String,
    /// 展示口径（预留：账单/收据文案用），入账金额以回调 money 为准
    #[allow(dead_code)]
    pub currency: String,
}

pub async fn gateway_config(state: &AppState) -> GatewayConfig {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name LIKE 'payment\\_%'",
    )
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    let m: std::collections::HashMap<String, String> =
        rows.into_iter().collect();
    let get = |k: &str| m.get(k).cloned().unwrap_or_default();
    GatewayConfig {
        provider: get("payment_provider"),
        gateway_url: get("payment_gateway_url")
            .trim()
            .trim_end_matches('/')
            .to_string(),
        pid: get("payment_pid"),
        key: get("payment_key"),
        currency: {
            let c = get("payment_currency");
            if c.is_empty() {
                "CNY".into()
            } else {
                c
            }
        },
    }
}

impl GatewayConfig {
    /// 通道可用：provider=epay 且三要素齐全
    pub fn available(&self) -> bool {
        self.provider == "epay"
            && !self.gateway_url.is_empty()
            && !self.pid.is_empty()
            && !self.key.is_empty()
    }
}

/// 支付网关抽象：新网关实现此 trait 并在 `create_payment_url` 分发。
pub trait PaymentProvider {
    /// 生成支付跳转 URL（把用户送去网关收银台）
    fn pay_url(
        &self,
        order_no: &str,
        amount_paid: &str,
        channel: &str,
        subject: &str,
        return_url: &str,
        notify_url: &str,
    ) -> String;
    /// 异步通知验签：合法返回网关流水号与实付金额（trade_no, amount_paid, order_no）
    fn verify_notify(
        &self,
        params: &std::collections::HashMap<String, String>,
    ) -> Option<NotifyData>;
}

#[derive(Debug, PartialEq)]
pub struct NotifyData {
    pub order_no: String,
    pub trade_no: String,
    pub amount_paid: String,
    pub status_ok: bool,
}

/// 按 site_settings 构造当前 Provider
pub fn provider_from(
    cfg: &GatewayConfig,
) -> Box<dyn PaymentProvider + Send + Sync> {
    // 目前唯一实现；新网关在此分发
    Box::new(EpayProvider {
        gateway_url: cfg.gateway_url.clone(),
        pid: cfg.pid.clone(),
        key: cfg.key.clone(),
    })
}
