//! webhook 出网判据与签名的单测。
//!
//! 这些判据是**安全护栏**，测试的重点不是「正常路径能用」，而是
//! 「攻击路径被挡住」。所以下面大量用例都在断言某个恶意输入必须被拒。

use super::*;

fn allow(url: &str) -> bool {
    matches!(egress_allowed(url, &[]), Egress::Allow)
}

fn deny(url: &str) -> bool {
    matches!(egress_allowed(url, &[]), Egress::Deny(_))
}

// ── SSRF：内网与云元数据必须被拒 ──────────────────────────────────

#[test]
fn cloud_metadata_is_blocked() {
    // 云厂商元数据端点：拿到的是 IAM 临时凭据，能直接接管云账号。
    assert!(deny("http://169.254.169.254/latest/meta-data/iam/"));
    assert!(deny("https://169.254.169.254/"));
}

#[test]
fn private_ranges_are_blocked() {
    for u in [
        "https://10.0.0.1/",
        "https://192.168.1.1/admin",
        "https://172.16.5.4/",
        "https://127.0.0.1:8080/",
        "https://[::1]/",
        "https://0.0.0.0/",
    ] {
        assert!(deny(u), "内网地址必须被拒：{u}");
    }
}

#[test]
fn cgnat_range_is_blocked_without_whitelist() {
    // 盒子常用 100.64/10。不当公网放行，但可用白名单显式放行。
    assert!(deny("https://100.100.1.1/"));
    assert!(matches!(
        egress_allowed("https://100.100.1.1/", &["100.64.0.0/10".into()]),
        Egress::Allow
    ));
}

#[test]
fn ipv6_unique_local_is_blocked() {
    assert!(deny("https://[fd00::1]/"));
    assert!(deny("https://[fe80::1]/"));
    // IPv4-mapped：::ffff:169.254.169.254 绕回云元数据
    assert!(deny("https://[::ffff:169.254.169.254]/"));
}

#[test]
fn localhost_names_are_blocked() {
    assert!(deny("https://localhost:5432/"));
    assert!(deny("https://foo.localhost/"));
    assert!(deny("https://db.internal/health"));
}

#[test]
fn non_http_schemes_are_blocked() {
    // file:// 能读本机文件；gopher:// 在旧栈上能走私站请求。
    for u in ["file:///etc/passwd", "gopher://127.0.0.1:6379/_SET"] {
        assert!(deny(u), "协议必须被拒：{u}");
    }
}

#[test]
fn malformed_url_is_blocked() {
    assert!(deny("not a url"));
    assert!(deny(""));
    assert!(deny("https://"));
}

#[test]
fn public_https_is_allowed() {
    assert!(allow("https://discord.com/api/webhooks/123/abc"));
    assert!(allow("https://8.8.8.8/hook"));
}

#[test]
fn public_http_requires_https() {
    // 公网 http：告警内容可被中间人改写，且收方无法验证来源。
    assert!(deny("http://8.8.8.8/hook"));
}

// ── 白名单：显式登记才放行，且写错不得变成全放行 ──────────────────

#[test]
fn whitelist_allows_registered_cidr() {
    let rules = vec!["192.168.10.0/24".to_string()];
    assert!(matches!(
        egress_allowed("http://192.168.10.5:9000/hook", &rules),
        Egress::Allow
    ));
}

#[test]
fn whitelist_does_not_leak_outside_its_cidr() {
    let rules = vec!["192.168.10.0/24".to_string()];
    // 白名单里的网段之外仍要按原判据走
    assert!(deny_with("https://192.168.11.1/", &rules));
    assert!(deny_with("https://169.254.169.254/", &rules));
}

#[test]
fn malformed_whitelist_never_allows_everything() {
    // 白名单写错时必须「不放行」，绝不能因解析失败变成全放行。
    let rules = vec!["不是CIDR".to_string(), "10.0.0.0/999".to_string()];
    assert!(deny_with("https://169.254.169.254/", &rules));
    assert!(deny_with("https://192.168.1.1/", &rules));
}

fn deny_with(url: &str, rules: &[String]) -> bool {
    matches!(egress_allowed(url, rules), Egress::Deny(_))
}

// ── 签名 ──────────────────────────────────────────────────────────

#[test]
fn empty_secret_means_no_signature() {
    // 兼容老站：密钥为空不签名，但也不能凭空造出签名。
    assert_eq!(sign("", 1_700_000_000, "body"), None);
    assert!(verify("", 1_700_000_000, "body", "", 1_700_000_000, 300));
}

#[test]
fn signature_is_deterministic_and_prefixed() {
    let s = sign("k", 100, "hello").unwrap();
    assert_eq!(s, sign("k", 100, "hello").unwrap());
    assert!(s.starts_with("sha256="));
    assert_eq!(s.len(), "sha256=".len() + 64);
}

#[test]
fn signature_changes_with_every_input() {
    let base = sign("k", 100, "body").unwrap();
    assert_ne!(base, sign("k2", 100, "body").unwrap(), "密钥不同");
    assert_ne!(base, sign("k", 101, "body").unwrap(), "时间不同");
    assert_ne!(base, sign("k", 100, "body2").unwrap(), "正文不同");
}

#[test]
fn verify_accepts_valid_and_rejects_tampered() {
    let ts = 1_700_000_000;
    let s = sign("secret", ts, "msg").unwrap();
    assert!(verify("secret", ts, "msg", &s, ts, 300));
    // 正文被改
    assert!(!verify("secret", ts, "msg-tampered", &s, ts, 300));
    // 密钥不同
    assert!(!verify("other", ts, "msg", &s, ts, 300));
    // 签名被改
    assert!(!verify("secret", ts, "msg", "sha256=deadbeef", ts, 300));
}

#[test]
fn verify_rejects_replay_outside_window() {
    let ts = 1_700_000_000;
    let s = sign("secret", ts, "msg").unwrap();
    // 窗口内
    assert!(verify("secret", ts, "msg", &s, ts + 299, 300));
    // 窗口外：旧签名重放
    assert!(!verify("secret", ts, "msg", &s, ts + 301, 300));
    // 未来时间戳同样拒绝（否则可预生成签名长期有效）
    assert!(!verify("secret", ts, "msg", &s, ts - 301, 300));
}

#[test]
fn cidr_parsing_is_fail_closed() {
    let ip: std::net::IpAddr = "10.1.2.3".parse().unwrap();
    assert!(!cidr_contains("garbage", ip));
    assert!(cidr_contains("10.0.0.0/8", ip));
    assert!(!cidr_contains("192.168.0.0/16", ip));
    // IPv6 白名单同样工作
    let ip6: std::net::IpAddr = "fd00::5".parse().unwrap();
    assert!(cidr_contains("fd00::/8", ip6));
}

#[test]
fn redirect_must_not_be_followed() {
    // 出网判据只看**首跳** URL。若客户端跟随重定向，一个白名单域名
    // 302 到 169.254.169.254，整条 SSRF 护栏就形同虚设。
    // 这条断言守住 ops_webhook.rs 里两个客户端的
    // redirect(reqwest::redirect::Policy::none())——删掉即红。
    let src = include_str!("../../ops_webhook.rs");
    // 只数代码行，不数注释（注释里也会提到 Policy::none()）。
    let code: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let built = code.matches("Client::builder()").count();
    let guarded = code.matches("Policy::none()").count();
    assert!(built >= 2, "应有至少两个自建客户端，实际 {built}");
    assert_eq!(
        built, guarded,
        "每个自建客户端都必须显式禁跟随重定向（否则可 302 绕过 SSRF 判据）"
    );
}

/// P-455 补的四段：v4 组播、0/8、240/4 与 v6 组播。
/// 此前只有 push 那份副本认得它们——两份规则并存的代价正落在这里。
#[test]
fn multicast_and_reserved_v4_are_forbidden() {
    use std::net::IpAddr;
    let f = |s: &str| super::is_forbidden(s.parse::<IpAddr>().unwrap());
    assert!(f("224.0.0.1"), "v4 组播");
    assert!(f("239.255.255.255"), "v4 管理范围组播");
    assert!(f("0.0.0.0"), "0/8 this-network");
    assert!(f("0.1.2.3"), "0/8 非全零也算内网");
    assert!(f("240.0.0.1"), "240/4 保留");
    assert!(f("ff02::1"), "v6 链路本地组播");
    assert!(
        !f("8.8.8.8") && !f("2001:4860:4860::8888"),
        "公网仍要放行，否则这四条断言是恒真的"
    );
}
