use super::*;

fn epay() -> EpayProvider {
    EpayProvider {
        gateway_url: "https://pay.example.com".into(),
        pid: "1001".into(),
        key: "testkey".into(),
    }
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
    let url = epay().pay_url(
        "flux-1-123",
        "10.00",
        "alipay",
        "捐赠",
        "https://s/ok",
        "https://s/notify",
    );
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
        ("pid", "1001"),
        ("trade_no", "E20260918001"),
        ("out_trade_no", "flux-1-123"),
        ("type", "alipay"),
        ("name", "捐赠"),
        ("money", "10.00"),
        ("trade_status", "TRADE_SUCCESS"),
    ];
    let sign = epay_sign(&base, "testkey");
    let mut m: std::collections::HashMap<String, String> = base
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    m.insert("sign".into(), sign.clone());
    let ok = p.verify_notify(&m).expect("合法签名应通过");
    assert_eq!(ok.order_no, "flux-1-123");
    assert!(ok.status_ok);

    // 篡改金额 → 拒绝
    m.insert("money".into(), "9999.00".into());
    assert!(p.verify_notify(&m).is_none(), "篡改金额必须拒绝");

    // 状态非成功 → 验签过但 status_ok=false
    let mut m2: std::collections::HashMap<String, String> = base
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    m2.insert("trade_status".into(), "WAIT_BUYER_PAY".into());
    m2.insert(
        "sign".into(),
        epay_sign(
            &[
                ("pid", "1001"),
                ("trade_no", "E20260918001"),
                ("out_trade_no", "flux-1-123"),
                ("type", "alipay"),
                ("name", "捐赠"),
                ("money", "10.00"),
                ("trade_status", "WAIT_BUYER_PAY"),
            ],
            "testkey",
        ),
    );
    let r = p.verify_notify(&m2).expect("签名本身合法");
    assert!(!r.status_ok);
}

/// 未配置 = 不可用（堵漏语义的配置化延续）
#[test]
fn gateway_unavailable_when_unconfigured() {
    let cfg = GatewayConfig {
        provider: "none".into(),
        gateway_url: String::new(),
        pid: String::new(),
        key: String::new(),
        currency: "CNY".into(),
    };
    assert!(!cfg.available());
    let cfg2 = GatewayConfig {
        provider: "epay".into(),
        gateway_url: "https://p".into(),
        pid: "1".into(),
        key: String::new(),
        currency: "CNY".into(),
    };
    assert!(!cfg2.available(), "缺密钥同样不可用");
}

#[test]
fn order_no_shape() {
    let n = new_order_no(7);
    assert!(n.starts_with("flux-7-"));
}
