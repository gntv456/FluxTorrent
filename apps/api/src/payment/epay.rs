//! 易支付协议族实现（submit.php / mapi.php 通用口径）。

use md5::{Digest, Md5};

use super::{NotifyData, PaymentProvider};

/// 易支付参数签名（MD5）：参数名 ASCII 升序 & 拼接（跳过空值/sign/sign_type）+ 密钥。
/// 独立函数便于单测锁定口径。
pub fn epay_sign(params: &[(&str, &str)], key: &str) -> String {
    let mut sorted: Vec<(String, String)> = params
        .iter()
        .filter(|(k, v)| {
            !k.is_empty() && *v != "" && *k != "sign" && *k != "sign_type"
        })
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    sorted.sort();
    let joined: Vec<String> =
        sorted.iter().map(|(k, v)| format!("{k}={v}")).collect();
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
    fn pay_url(
        &self,
        order_no: &str,
        amount_paid: &str,
        channel: &str,
        subject: &str,
        return_url: &str,
        notify_url: &str,
    ) -> String {
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

    fn verify_notify(
        &self,
        params: &std::collections::HashMap<String, String>,
    ) -> Option<NotifyData> {
        // 口径：sign = MD5(升序&拼接(除 sign/sign_type/空值) + key)；
        // trade_status=TRADE_SUCCESS
        let vec: Vec<(String, String)> = params
            .iter()
            .filter(|(k, v)| {
                k.as_str() != "sign"
                    && k.as_str() != "sign_type"
                    && !v.is_empty()
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let expect = {
            let mut sorted = vec.clone();
            sorted.sort();
            let joined: Vec<String> =
                sorted.iter().map(|(k, v)| format!("{k}={v}")).collect();
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
            status_ok: params
                .get("trade_status")
                .map(|s| s == "TRADE_SUCCESS")
                .unwrap_or(false),
        })
    }
}

/// 简易 URL 编码（支付参数只需覆盖 & = ? % 中文与空格）
fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
