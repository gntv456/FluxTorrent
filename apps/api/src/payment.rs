//! 支付网关抽象（U4 §12.1）：Provider trait + 易支付协议族实现。
//!
//! 设计纪律：
//! - 网关凭证全部在 site_settings（payment_*，密钥 secret 型后台回掩码），代码不绑商户；
//! - 未配置（provider=none / 凭证缺失）= 通道未开通：topup 拒单、前端显示「通道未开通」；
//! - 回调验签 + order_no 幂等（payment_orders 唯一键 + donation_ledger 复用入账）；
//! - 仅实现「跳转支付」（submit.php?...）形态：最通用、无异步通知服务器要求，
//!   同步回跳由前端轮询订单状态；异步 notify 端点同样实现（get 请求 + MD5 验签）。

use md5::{Md5, Digest};

use crate::errors::{DomainError, DomainResult};
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
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT name, value FROM site_settings WHERE name LIKE 'payment\\_%'")
            .fetch_all(&state.repo.db)
            .await
            .unwrap_or_default();
    let m: std::collections::HashMap<String, String> = rows.into_iter().collect();
    let get = |k: &str| m.get(k).cloned().unwrap_or_default();
    GatewayConfig {
        provider: get("payment_provider"),
        gateway_url: get("payment_gateway_url").trim().trim_end_matches('/').to_string(),
        pid: get("payment_pid"),
        key: get("payment_key"),
        currency: { let c = get("payment_currency"); if c.is_empty() { "CNY".into() } else { c } },
    }
}

impl GatewayConfig {
    /// 通道可用：provider=epay 且三要素齐全
    pub fn available(&self) -> bool {
        self.provider == "epay" && !self.gateway_url.is_empty() && !self.pid.is_empty() && !self.key.is_empty()
    }
}

/// 支付网关抽象：新网关实现此 trait 并在 `create_payment_url` 分发。
pub trait PaymentProvider {
    /// 生成支付跳转 URL（把用户送去网关收银台）
    fn pay_url(&self, order_no: &str, amount_paid: &str, channel: &str, subject: &str, return_url: &str, notify_url: &str) -> String;
    /// 异步通知验签：合法返回网关流水号与实付金额（trade_no, amount_paid, order_no）
    fn verify_notify(&self, params: &std::collections::HashMap<String, String>) -> Option<NotifyData>;
}

#[derive(Debug, PartialEq)]
pub struct NotifyData {
    pub order_no: String,
    pub trade_no: String,
    pub amount_paid: String,
    pub status_ok: bool,
}

// ============ 易支付协议族（submit.php / mapi.php 通用口径） ============

/// 易支付参数签名（MD5）：参数名 ASCII 升序 & 拼接（跳过空值/sign/sign_type）+ 密钥。
/// 独立函数便于单测锁定口径。
pub fn epay_sign(params: &[(&str, &str)], key: &str) -> String {
    let mut sorted: Vec<(String, String)> = params
        .iter()
        .filter(|(k, v)| !k.is_empty() && *v != "" && *k != "sign" && *k != "sign_type")
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    sorted.sort();
    let joined: Vec<String> = sorted.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let raw = joined.join("&");
    let mut hasher = Md5::new();
    hasher.update(raw.as_bytes());
    hasher.update(key.as_bytes());
    hex(&hasher.finalize())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 易支付实现
pub struct EpayProvider {
    pub gateway_url: String,
    pub pid: String,
    pub key: String,
}

impl PaymentProvider for EpayProvider {
    fn pay_url(&self, order_no: &str, amount_paid: &str, channel: &str, subject: &str, return_url: &str, notify_url: &str) -> String {
        let params: Vec<(&str, &str)> = vec![
            ("pid", self.pid.as_str()),
            ("type", channel),
            ("out_trade_no", order_no),
            ("notify_url", notify_url),
            ("return_url", return_url),
            ("name", subject),
            ("money", amount_paid),
        ];
        let sign = epay_sign(&params, &self.key);
        let q: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{k}={}", urlencode(v)))
            .chain(std::iter::once(format!("sign={sign}")))
            .chain(std::iter::once("sign_type=MD5".into()))
            .collect();
        format!("{}/submit.php?{}", self.gateway_url, q.join("&"))
    }

    fn verify_notify(&self, params: &std::collections::HashMap<String, String>) -> Option<NotifyData> {
        // 口径：sign = MD5(升序&拼接(除 sign/sign_type/空值) + key)；trade_status=TRADE_SUCCESS
        let vec: Vec<(String, String)> = params
            .iter()
            .filter(|(k, v)| k.as_str() != "sign" && k.as_str() != "sign_type" && !v.is_empty())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let expect = {
            let mut sorted = vec.clone();
            sorted.sort();
            let joined: Vec<String> = sorted.iter().map(|(k, v)| format!("{k}={v}")).collect();
            let raw = joined.join("&");
            let mut hasher = Md5::new();
            hasher.update(raw.as_bytes());
            hasher.update(self.key.as_bytes());
            hex(&hasher.finalize())
        };
        if params.get("sign").map(|s| s.as_str()) != Some(expect.as_str()) {
            return None;
        }
        Some(NotifyData {
            order_no: params.get("out_trade_no")?.clone(),
            trade_no: params.get("trade_no").cloned().unwrap_or_default(),
            amount_paid: params.get("money").cloned().unwrap_or_default(),
            status_ok: params.get("trade_status").map(|s| s == "TRADE_SUCCESS").unwrap_or(false),
        })
    }
}

/// 简易 URL 编码（支付参数只需覆盖 & = ? % 中文与空格）
fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// 按 site_settings 构造当前 Provider
pub fn provider_from(cfg: &GatewayConfig) -> Box<dyn PaymentProvider + Send + Sync> {
    // 目前唯一实现；新网关在此分发
    Box::new(EpayProvider {
        gateway_url: cfg.gateway_url.clone(),
        pid: cfg.pid.clone(),
        key: cfg.key.clone(),
    })
}

/// 站内单号（幂等键）：flux-{uid}-{纳秒}（同用户重复点击各生成独立订单，回调按单号幂等）
pub fn new_order_no(user_id: i64) -> String {
    format!("flux-{}-{}", user_id, chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default())
}

/// 回调统一入口：验签 + 幂等入账（wallet_usd 增加 + donation_ledger + 订单 paid）
pub async fn settle_notify(
    state: &AppState,
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
        "UPDATE payment_orders SET status='paid', trade_no=$2, amount_paid=$3, paid_at=now() \
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
        "UPDATE users SET wallet_usd = wallet_usd + $2, donor = true WHERE id = $1 RETURNING wallet_usd::float8",
    )
    .bind(user_id)
    .bind(amount)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO donation_ledger (user_id, kind, amount_usd, balance_after, note) \
         VALUES ($1, 'topup', $2, $3, $4)",
    )
    .bind(user_id)
    .bind(amount)
    .bind(balance)
    .bind(format!("epay 回调入账 trade_no={}", n.trade_no))
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
    Ok(SettleOutcome::Paid)
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
        "UPDATE users SET wallet_usd = wallet_usd + $2, donor = true WHERE id = $1 RETURNING wallet_usd::float8",
    )
    .bind(user_id)
    .bind(amount_usd)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO donation_ledger (user_id, kind, amount_usd, balance_after, note) VALUES ($1, 'topup', $2, $3, $4)",
    )
    .bind(user_id)
    .bind(amount_usd)
    .bind(balance)
    .bind(format!("模拟支付成功（{channel}）"))
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit().await.map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(user_id), "donate_topup_demo", None).await;
    Ok(serde_json::json!({ "wallet_usd": balance }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn epay() -> EpayProvider {
        EpayProvider { gateway_url: "https://pay.example.com".into(), pid: "1001".into(), key: "testkey".into() }
    }

    /// 签名口径锁定：升序 & 拼接 + 密钥 MD5
    #[test]
    fn epay_sign_deterministic() {
        let params = [("b", "2"), ("a", "1")];
        let s1 = epay_sign(&params, "k");
        let s2 = epay_sign(&[("a", "1"), ("b", "2")], "k");
        assert_eq!(s1, s2, "参数顺序不影响签名");
        assert_eq!(s1.len(), 32);
    }

    /// 跳转 URL 形状
    #[test]
    fn pay_url_shape() {
        let url = epay().pay_url("flux-1-123", "10.00", "alipay", "捐赠", "https://s/ok", "https://s/notify");
        assert!(url.starts_with("https://pay.example.com/submit.php?"));
        assert!(url.contains("out_trade_no=flux-1-123"));
        assert!(url.contains("sign_type=MD5"));
        assert!(url.contains("money=10.00"));
    }

    /// 验签：合法通过 / 篡改拒绝 / 状态非成功
    #[test]
    fn verify_notify_cases() {
        let p = epay();
        let base = [
            ("pid", "1001"), ("trade_no", "E20260918001"), ("out_trade_no", "flux-1-123"),
            ("type", "alipay"), ("name", "捐赠"), ("money", "10.00"),
            ("trade_status", "TRADE_SUCCESS"),
        ];
        let sign = epay_sign(&base, "testkey");
        let mut m: std::collections::HashMap<String, String> =
            base.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        m.insert("sign".into(), sign.clone());
        let ok = p.verify_notify(&m).expect("合法签名应通过");
        assert_eq!(ok.order_no, "flux-1-123");
        assert!(ok.status_ok);

        // 篡改金额 → 拒绝
        m.insert("money".into(), "9999.00".into());
        assert!(p.verify_notify(&m).is_none(), "篡改金额必须拒绝");

        // 状态非成功 → 验签过但 status_ok=false
        let mut m2: std::collections::HashMap<String, String> =
            base.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        m2.insert("trade_status".into(), "WAIT_BUYER_PAY".into());
        m2.insert("sign".into(), epay_sign(&[("pid","1001"),("trade_no","E20260918001"),("out_trade_no","flux-1-123"),("type","alipay"),("name","捐赠"),("money","10.00"),("trade_status","WAIT_BUYER_PAY")], "testkey"));
        let r = p.verify_notify(&m2).expect("签名本身合法");
        assert!(!r.status_ok);
    }

    /// 未配置 = 不可用（堵漏语义的配置化延续）
    #[test]
    fn gateway_unavailable_when_unconfigured() {
        let cfg = GatewayConfig { provider: "none".into(), gateway_url: String::new(), pid: String::new(), key: String::new(), currency: "CNY".into() };
        assert!(!cfg.available());
        let cfg2 = GatewayConfig { provider: "epay".into(), gateway_url: "https://p".into(), pid: "1".into(), key: String::new(), currency: "CNY".into() };
        assert!(!cfg2.available(), "缺密钥同样不可用");
    }

    #[test]
    fn order_no_shape() {
        let n = new_order_no(7);
        assert!(n.starts_with("flux-7-"));
    }
}
