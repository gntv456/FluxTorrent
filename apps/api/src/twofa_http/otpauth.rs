//! otpauth URI 装配（从 twofa_http.rs 拆出，300 门禁）。
//!
//! P-106 现场：旧实现拼出 `otpauth://totp/:4867?secret=…&issuer=`——issuer
//! 是空串（现网 `PUBLIC_SITE_NAME=` 空值绕过了 unwrap_or_else 兜底），标签
//! 又是数字用户号：验证器 App 里既看不出站点、也看不出是哪个账号。
//! 现在 issuer 取设置中心的 `site_name`（与邮件/页面同一份），env 仅在非空
//! 时兜底；标签用用户名；两段都做百分号编码（站点名可含中文与空格）。

use sqlx::PgPool;

/// URI 组件百分号编码：非保留字符一律转义（按 utf-8 字节）
pub fn pct(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        let c = *b as char;
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
            out.push(c);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// 展示名（issuer）：`site_settings.site_name` → `PUBLIC_SITE_NAME`（非空）
/// → 通用兜底。空串一律按「未配置」处理——正是旧写法把空 env 当有效值。
pub async fn issuer(db: &PgPool) -> String {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'site_name'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
    .or_else(|| {
        std::env::var("PUBLIC_SITE_NAME")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    })
    .unwrap_or_else(|| "FluxTorrent".into())
}

/// `otpauth://totp/{issuer}:{account}?secret=…&issuer=…`
pub fn uri(issuer: &str, account: &str, secret_b32: &str) -> String {
    format!(
        "otpauth://totp/{}?secret={}&issuer={}",
        pct(&format!("{issuer}:{account}")),
        secret_b32,
        pct(issuer)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pct_escapes_non_unreserved() {
        assert_eq!(pct("abC0-_.~"), "abC0-_.~");
        assert_eq!(pct("a b"), "a%20b");
        assert_eq!(pct("好"), "%E5%A5%BD");
    }

    #[test]
    fn uri_uses_account_label_and_encoded_issuer() {
        let u = uri("同类站堂 PT", "donna", "JBSWY3DP");
        assert!(u.starts_with("otpauth://totp/"));
        assert!(u.contains(":donna?secret=JBSWY3DP"), "标签是用户名: {u}");
        assert!(u.contains("%E5%A5%BD"), "站点名要转义: {u}");
        assert!(u.contains("%20PT"), "空格要转义: {u}");
        assert!(!u.contains(' '), "URI 里不得留裸空格: {u}");
    }
}
