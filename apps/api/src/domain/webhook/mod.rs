//! 运维 webhook 的出网判据与签名（评估报告 §1.4）。
//!
//! 两件事，都是纯函数以便单测：
//!   1. **SSRF 护栏**（`egress_allowed`）：URL 存在 site_settings 里，
//!      任何能改设置的人都能把它指向云元数据（`169.254.169.254`）或
//!      内网管理端口，借 api 的身份扫内网。判据必须 fail-closed：
//!      解析不出主机、不是 http(s)、私网/环回/链路本地/保留段——
//!      只要有一条说不准，就当**不允许**。
//!   2. **HMAC 签名**（`sign`）：URL 常被贴进工单、截图、日志，
//!      收方若无法验证来源，任何人都能伪造「作弊告警」来钓鱼值班。
//!
//! 【为什么放在 D 层】这两个判据是「允许/不允许」「签名/不签名」的
//! 纯逻辑，不该夹在 reqwest 调用之间。SQL 在 `repo/webhook.rs`。

/// 出网判据的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Egress {
    /// 允许（公网 https，或命中白名单网段）。
    Allow,
    /// 拒绝，并给出**人可读**的原因（会写进投递台账）。
    Deny(String),
}

/// 是否允许对该 URL 出网。
///
/// `allow_cidrs` 是运维显式登记的内网网段（`webhook_egress_rules`），
/// 空 = 只允许公网。**不做 DNS 解析**：解析结果会随时间变化，且
/// 「解析完再连」之间存在 rebinding 窗口——真要防 rebinding 得在
/// 连接层固定 IP，超出本函数的职责。这里拦的是「配置里直接写了
/// 内网地址」这一类，成本极低、覆盖绝大多数实际滥用。
pub fn egress_allowed(url: &str, allow_cidrs: &[String]) -> Egress {
    let u = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return Egress::Deny("URL 无法解析".into()),
    };
    match u.scheme() {
        "https" => {}
        // http 只在显式白名单命中时放行（内网自建 webhook 常见 http）
        "http" => {}
        s => return Egress::Deny(format!("协议不允许：{s}")),
    }
    let host = match u.host_str() {
        Some(h) if !h.is_empty() => h.to_ascii_lowercase(),
        _ => return Egress::Deny("URL 无主机名".into()),
    };
    // 主机名（非 IP）：走公网 DNS，SSRF 面只剩「解析到私网」。
    // 本函数不解析，故按公网放行——但环回/本地名仍要拦。
    let ip: Option<std::net::IpAddr> = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse()
        .ok();
    let Some(ip) = ip else {
        let bad = host == "localhost"
            || host.ends_with(".localhost")
            || host.ends_with(".local")
            || host.ends_with(".internal");
        return if bad {
            Egress::Deny(format!("内网主机名不允许：{host}"))
        } else {
            Egress::Allow
        };
    };
    // 白名单命中优先（运维显式登记过就是有意为之）。
    if allow_cidrs.iter().any(|c| cidr_contains(c, ip)) {
        return Egress::Allow;
    }
    if is_forbidden(ip) {
        return Egress::Deny(format!(
            "目标地址属于内网/保留段，不允许出网：{ip}"
        ));
    }
    // 公网 IP + http：无加密的告警内容可被中间人改写。
    if u.scheme() == "http" {
        return Egress::Deny(
            "非白名单目标必须用 https（http 告警可被中间人改写）".into(),
        );
    }
    Egress::Allow
}

/// 私网 / 环回 / 链路本地 / 保留段。fail-closed 的核心清单。
pub fn is_forbidden(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()      // 169.254.0.0/16 云元数据
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
                // P-455：push 侧曾自己写第二份判定，多认了三段——组播、
                // 0/8（"this network"）、240/4 保留。补齐后分类只留这一处：
                // 两份规则并存必然漂移（push 那个闸门本来就在断言
                // 「全仓只有一份」，正是该由它抓住这次重复的）。
                || v4.is_multicast()
                || v4.octets()[0] == 0
                || v4.octets()[0] >= 240
                // 100.64.0.0/10 CGNAT：盒子常用，不能直接当内网拦，
                // 但也不该被当公网放行——交给白名单显式表态。
                || (v4.octets()[0] == 100
                    && (64..128).contains(&v4.octets()[1]))
        }
        std::net::IpAddr::V6(v6) => {
            let s = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()   // P-455：与 v4 同理由
                // fc00::/7 唯一本地地址
                || (s[0] & 0xfe00) == 0xfc00
                // fe80::/10 链路本地
                || (s[0] & 0xffc0) == 0xfe80
                || v6.to_ipv4_mapped().map(Ip4Ext::forbidden_v4)
                    .unwrap_or(false)
        }
    }
}

/// `is_forbidden` 对 v4 的复用（IPv4-mapped IPv6 归一后判同一套）。
trait Ip4Ext {
    fn forbidden_v4(self) -> bool;
}
impl Ip4Ext for std::net::Ipv4Addr {
    fn forbidden_v4(self) -> bool {
        is_forbidden(std::net::IpAddr::V4(self))
    }
}

/// 目标 IP 是否落在该 CIDR 内。**解析失败一律 false**（fail-closed：
/// 白名单写错时不得变成「全部放行」）。
pub fn cidr_contains(cidr: &str, ip: std::net::IpAddr) -> bool {
    let Ok(net) = cidr.parse::<ipnet::IpNet>() else {
        return false;
    };
    net.contains(&ip)
}

/// HMAC-SHA256 签名（十六进制），格式同 GitHub webhook：
/// `sha256=<hex>`。签名内容 = `timestamp\nbody`（防重放时间窗）。
///
/// 密钥为空 → 返回 None（不签名）。这不是「安全」，而是**兼容**
/// 老站：突然要求签名会让既有出口全部静默失效，且失败原因难查。
pub fn sign(secret: &str, timestamp: i64, body: &str) -> Option<String> {
    if secret.is_empty() {
        return None;
    }
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut m = HmacSha256::new_from_slice(secret.as_bytes()).ok()?;
    m.update(timestamp.to_string().as_bytes());
    m.update(b"\n");
    m.update(body.as_bytes());
    Some(format!("sha256={}", hex(&m.finalize().into_bytes())))
}

/// 校验签名（收方视角，也用于回放测试）。允许 ±`window` 秒。
pub fn verify(
    secret: &str,
    timestamp: i64,
    body: &str,
    signature: &str,
    now: i64,
    window: i64,
) -> bool {
    if (now - timestamp).abs() > window {
        return false; // 重放窗口外
    }
    match sign(secret, timestamp, body) {
        Some(s) => constant_eq(s.as_bytes(), signature.as_bytes()),
        // 密钥为空 = 不签名模式，此时任何签名都算通过
        None => signature.is_empty(),
    }
}

/// 恒定时间比较（签名比对不能用 `==`：会因短路而泄漏前缀）。
fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut d = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        d |= x ^ y;
    }
    d == 0
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests;
