//! 客户端 IP 的取信与校验（审计 10-07 P1-3 从 helpers.rs 拆出，300 行门禁）。
//!
//! 三条来源，优先级：XFF 右数第 DEPTH 段 > `?ip=` 参数 > socket 对端。
//! 前两条是**客户端可写**的，取值必须能解析成地址且不属于保留/内网段，
//! 否则整条防线等于没有：实测过三种打穿方式
//!   · `X-Forwarded-For: garbage` → 封禁绕过 + 限流键被写成任意串
//!   · `X-Forwarded-For: 8.8.8.8` → 他人 peer 列表被投毒
//!   · `?ip=172.20.0.5&port=5432` → 内网地址进 peer 表并下发全站，
//!     而 tracker 每 5min 的 connectable 回连抽样会主动 TCP 连过去（实测 conn=1）

use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};

fn trust_xff() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("TRUST_PROXY").unwrap_or_default() == "1")
}

/// 缺省 1 = 单层反代（与旧「右值」语义兼容）；多层代理部署须显式对齐拓扑：
/// DEPTH=n 时取右数第 n 段（攻击者自建代理追加 hop 时右值可被伪造污染取证/绕限流）
fn trust_xff_depth() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("TRUST_PROXY_DEPTH")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(1)
            .clamp(1, 8)
    })
}

fn trust_param_ip() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("TRUST_PROXY_IP").unwrap_or_default() == "1"
    })
}

/// 内网/保留段自报地址是否放行。默认关：纯内网部署（校园网/机房内部
/// tracker，客户端本来就是 10.x/172.x）才需要显式开这一档。
fn allow_private_peer_ip() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("ALLOW_PRIVATE_PEER_IP").unwrap_or_default() == "1"
    })
}

/// IPv4 映射的 IPv6（`::ffff:a.b.c.d`）降级成 v4：否则同一个人被当成 v6 peer
/// 进 `peers6`，纯 v4 客户端反而拿不到他。
fn normalize_ip(ip: IpAddr) -> IpAddr {
    if let IpAddr::V6(v6) = ip {
        if let Some(v4) = v6.to_ipv4_mapped() {
            return IpAddr::V4(v4);
        }
    }
    ip
}

/// 文档保留段（RFC5737 v4 / RFC3849 v6）。`IpAddr::is_documentation`
/// 至今仍是 unstable（rust issue #27709），本函数用显式 CIDR 判定等价替换：
///   v4: 192.0.2.0/24、198.51.100.0/24、203.0.113.0/24
///   v6: 2001:db8::/32
fn is_documentation(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            matches!(
                (o[0], o[1], o[2]),
                (192, 0, 2) | (198, 51, 100) | (203, 0, 113)
            )
        }
        IpAddr::V6(v6) => {
            let o = v6.octets();
            o[0] == 0x20 && o[1] == 0x01 && o[2] == 0x0d && o[3] == 0xb8
        }
    }
}

/// 该串能不能当作「客户端自报的地址」采信。
/// 永远拒绝：非 IP 字面量、本环、未指定、组播、文档段、链路本地、广播。
/// 默认还拒绝 RFC1918 / CGNAT / ULA，仅 `ALLOW_PRIVATE_PEER_IP=1` 放行。
pub(crate) fn injectable_ip(s: &str) -> Option<IpAddr> {
    let ip = normalize_ip(s.trim().parse::<IpAddr>().ok()?);
    if ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || is_documentation(&ip)
    {
        return None;
    }
    match ip {
        IpAddr::V4(v4) => {
            if v4.is_link_local() || v4.is_broadcast() {
                return None;
            }
            let o = v4.octets();
            // 100.64.0.0/10：运营商级 NAT 共享段，不是可回连的公网地址
            let cgnat = o[0] == 100 && (o[1] & 0xc0) == 64;
            if (v4.is_private() || cgnat) && !allow_private_peer_ip() {
                return None;
            }
        }
        IpAddr::V6(v6) => {
            if v6.is_unicast_link_local() {
                return None;
            }
            let o = v6.octets();
            let ula = (o[0] & 0xfe) == 0xfc; // fc00::/7
            if ula && !allow_private_peer_ip() {
                return None;
            }
        }
    }
    Some(ip)
}

/// 回连探测的目标过滤（纵深防御，与入表校验独立）：本环/未指定/组播/
/// 链路本地/文档段一律不回连——这些地址在任何部署里都不可能是真实对端。
/// RFC1918 不在这里拒绝：单机 docker 栈里 socket 对端本来就是 172.x，
/// 一刀切会让本地环境的 connectable 抽样整体失效（幽灵做种防线反而塌）。
pub(crate) fn probeable_ip(s: &str) -> bool {
    let ip = match s.trim().parse::<IpAddr>() {
        Ok(v) => normalize_ip(v),
        Err(_) => return false,
    };
    let link_local = match ip {
        IpAddr::V4(v4) => v4.is_link_local() || v4.is_broadcast(),
        IpAddr::V6(v6) => v6.is_unicast_link_local(),
    };
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || is_documentation(&ip)
        || link_local)
}

/// 被拒的自报 IP 计数。用进程内静态而非 `Metrics` 字段：后者要为多一个
/// 计数器去改 `client_ip` 的调用签名，而 announce 链路正被另一路改动。
static IP_INJECT_REJECTED: AtomicU64 = AtomicU64::new(0);

pub(crate) fn ip_inject_rejected() -> u64 {
    IP_INJECT_REJECTED.load(Ordering::Relaxed)
}

/// 客户端 IP 判定（announce/scrape 共用——审计 10-06 第 3 条：scrape 曾只认
/// socket 对端，LB 后全部 peer IP 变 LB 地址，与 announce 口径分叉）。
/// 取不到合法自报值时回落 socket 对端，绝不采信非法串。
pub(crate) fn client_ip(
    req: &actix_web::HttpRequest,
    params: &super::params::RawParams,
) -> String {
    let socket_ip = req
        .peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "0.0.0.0".into());
    let injected: Option<String> = if trust_xff() {
        let depth = trust_xff_depth();
        req.headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .map(|v| {
                v.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            })
            .filter(|parts| parts.len() >= depth)
            .and_then(|parts| {
                parts.get(parts.len() - depth).map(|s| s.to_string())
            })
    } else if trust_param_ip() {
        params.get_str("ip").filter(|s| !s.is_empty())
    } else {
        None
    };
    match injected {
        Some(raw) => match injectable_ip(&raw) {
            Some(ip) => ip.to_string(),
            None => {
                IP_INJECT_REJECTED.fetch_add(1, Ordering::Relaxed);
                socket_ip
            }
        },
        None => socket_ip,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garbage_and_reserved_are_not_injectable() {
        // 非 IP 字面量（旧版直接当字符串用 → 封禁绕过 + 限流键污染）
        assert!(injectable_ip("spoof042.example").is_none());
        assert!(injectable_ip("").is_none());
        assert!(injectable_ip("999.999.999.999").is_none());
        // 本环 / 未指定 / 组播 / 链路本地 / 文档段
        for s in [
            "127.0.0.1",
            "0.0.0.0",
            "224.0.0.1",
            "169.254.1.1",
            "192.0.2.1",
            "::1",
            "ff02::1",
            "fe80::1",
        ] {
            assert!(injectable_ip(s).is_none(), "{s} 不该被采信");
        }
    }

    #[test]
    fn internal_ranges_are_not_injectable_by_default() {
        // 实测攻击面：?ip=172.20.0.5 + port=5432 曾被直接写进 peer 表
        for s in [
            "172.20.0.5",
            "10.0.0.1",
            "192.168.1.7",
            "100.64.0.9",
            "fd00::1",
        ] {
            assert!(injectable_ip(s).is_none(), "{s} 是内网段");
        }
    }

    #[test]
    fn public_ips_are_accepted_and_normalized() {
        assert_eq!(
            injectable_ip(" 8.8.8.8 "),
            Some("8.8.8.8".parse().unwrap())
        );
        assert_eq!(
            injectable_ip("2001:4860:4860::8888"),
            Some("2001:4860:4860::8888".parse().unwrap())
        );
        // ::ffff:a.b.c.d 降级成 v4，否则会错进 peers6
        assert_eq!(
            injectable_ip("::ffff:8.8.4.4"),
            Some("8.8.4.4".parse().unwrap())
        );
    }

    #[test]
    fn probe_filter_blocks_only_never_valid_targets() {
        assert!(!probeable_ip("127.0.0.1"));
        assert!(!probeable_ip("::1"));
        assert!(!probeable_ip("fe80::2"));
        assert!(!probeable_ip("not-an-ip"));
        // 单机栈里 socket 对端就是 172.x —— 探测必须照常工作
        assert!(probeable_ip("172.20.0.1"));
        assert!(probeable_ip("8.8.8.8"));
    }
}
