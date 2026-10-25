//! Web Push 的出网闸门（P-453）：endpoint 是会员自带的 URL，站点会向它 POST
//! 加密载荷，所以这是一条**由会员控制的出网口**。
//!
//! 旧写法只在 subscribe 侧比字符串前缀（`starts_with("127.")`…），
//! `https://169-254-169-254.nip.io/`（公共通配 DNS，A 记录就是云元数据地址）、
//! 自建域指回 127.0.0.1、以及「URL 解析失败 ⇒ `if let Ok(..)` 跳过检查」
//! 三条路都能穿过去。这里把判定改成**解析后逐个 IP 判公网**，并且
//! 把校验过的地址钉进 HTTP client——不钉的话，「校验时一个答案、发送时
//! 另一个答案」的 DNS 重绑定照样能绕过白名单。

use std::net::{IpAddr, SocketAddr};

/// endpoint 的「主机 + 端口」提取：scheme 必须是 https，且**解析不出来就拒**。
/// 拆成纯函数是为了在没有异步运行时的单元测试里钉住它。
pub(crate) fn endpoint_host_port(endpoint: &str) -> Option<(String, u16)> {
    let u = url::Url::parse(endpoint).ok()?;
    if u.scheme() != "https" {
        return None;
    }
    let host = u.host_str()?;
    if host.is_empty() {
        return None;
    }
    Some((host.to_string(), u.port_or_known_default().unwrap_or(443)))
}

/// 一组地址是否**全部**公网（空集按不可信处理——解析失败不是放行理由）。
pub(crate) fn ips_public(addrs: &[SocketAddr]) -> bool {
    !addrs.is_empty() && addrs.iter().all(|a| is_public_ip(a.ip()))
}

/// IP 层「可出网」判定：**委托** `domain::webhook::is_forbidden`。
///
/// P-455：这里原来自己写了第二份（与运维 webhook 那套并行），两边各有
/// 缺段——那套不认 v4 组播 / 0/8 / 240/4（已补进去），这份不认
/// documentation 段与 fe80::/10。两份规则并存必然漂移，所以分类只留一处；
/// push 只保留自己更严的部分：scheme 只认 https、解析后**逐个**地址判、
/// 并把地址钉进 client（见 `guard_endpoint`）。
pub(crate) fn is_public_ip(ip: IpAddr) -> bool {
    !crate::domain::webhook::is_forbidden(ip)
}

/// endpoint 出网闸门：取主机端口（fail-close）→ 解析 → 逐 IP 判公网。
/// 返回 `(host, addrs)` 供调用方把地址钉进 HTTP client；`None` = 拒。
pub(crate) async fn guard_endpoint(
    endpoint: &str,
) -> Option<(String, Vec<SocketAddr>)> {
    let (host, port) = endpoint_host_port(endpoint)?;
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    let addrs: Vec<SocketAddr> = match bare.parse::<IpAddr>() {
        Ok(ip) => vec![SocketAddr::new(ip, port)],
        Err(_) => tokio::net::lookup_host((host.as_str(), port))
            .await
            .ok()?
            .collect(),
    };
    if !ips_public(&addrs) {
        return None;
    }
    Some((host, addrs))
}

#[cfg(test)]
mod tests {
    use super::{endpoint_host_port, ips_public, is_public_ip};
    use std::net::{IpAddr, SocketAddr};

    fn ip(s: &str) -> IpAddr {
        s.parse::<IpAddr>().expect(s)
    }

    fn sa(s: &str) -> SocketAddr {
        SocketAddr::new(ip(s), 443)
    }

    /// 内网/特殊用途一律不算公网：逐条点名，白名单写反就是默认放行。
    #[test]
    fn private_ranges_are_not_public() {
        for s in [
            "127.0.0.1",
            "10.0.0.9",
            "172.16.5.5",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "100.127.255.255",
            "0.0.0.0",
            "0.1.2.3",
            "224.0.0.1",
            "255.255.255.255",
            "240.0.0.1",
            "::1",
            "fc00::1",
            "fd12:3456::7",
            "::ffff:127.0.0.1",
            "::",
        ] {
            assert!(!is_public_ip(ip(s)), "{} 不能算公网", s);
        }
    }

    /// 公网侧也要有反例，否则「全判非公网」这种实现能骗过上一条。
    #[test]
    fn public_addresses_are_public() {
        for s in [
            "8.8.8.8",
            "1.1.1.1",
            "172.32.0.1",
            "100.63.0.1",
            "100.128.0.1",
            "2001:4860:4860::8888",
            "13.225.0.1",
        ] {
            assert!(is_public_ip(ip(s)), "{} 应是公网", s);
        }
    }

    /// 一条公网 + 一条内网 = 整组拒（DNS 返回多条 A 记录是常态）
    #[test]
    fn any_private_addr_rejects_the_group() {
        assert!(ips_public(&[sa("8.8.8.8"), sa("1.1.1.1")]));
        assert!(!ips_public(&[sa("8.8.8.8"), sa("127.0.0.1")]));
        assert!(!ips_public(&[]), "解析不出地址按不可信处理");
    }

    /// 入口 fail-close：解析不出来 / 非 https / 空主机 一律 None
    #[test]
    fn endpoint_parse_is_fail_close() {
        assert!(endpoint_host_port("不是个 URL").is_none());
        assert!(endpoint_host_port("http://web.push.example/x").is_none());
        // 「空主机」这类畸形 URL 不在 parse 层判（url crate 给的并不是
        // 空主机），它由下一步兜住：解析不出任何地址 ⇒ ips_public(空集)
        // = false ⇒ 拒。见 any_private_addr_rejects_the_group 里的空集断言。
        assert_eq!(
            endpoint_host_port("https://[::1]/x"),
            Some(("[::1]".to_string(), 443))
        );
        assert_eq!(
            endpoint_host_port("https://push.example/a").map(|x| x.0),
            Some("push.example".to_string())
        );
    }
}
