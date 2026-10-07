//! 防护缓存的派生状态：段封禁匹配、种子元数据、未注册负缓存、scrape 限流档
//! （审计 10-07 P2/P3 自 guard.rs 拆出，300 行门禁）。
//!
//! 这些状态放在进程内静态而非 `GuardInner` 字段：`GuardInner` 的初始化字面量在
//! main.rs，而 main.rs 此刻正被另一路改动。下一批合并回 `GuardInner`。

use std::net::IpAddr;
use std::sync::{OnceLock, RwLock};

/// 带掩码的封禁。`ip_bans.ip` 是 inet，PG 能存 `10.9.8.0/24`，但旧实现取值用
/// `host(ip)` 会把掩码剥掉 ⇒ 段封禁静默降级成「只封 10.9.8.0 这个主机」。
/// PT 现实是动态 IP：机房 /24、教育网 /64 复发，按段封才管用。
pub(crate) enum NetBan {
    V4 { net: u32, mask: u32, reason: String },
    V6 { net: u128, mask: u128, reason: String },
}

/// 解析 `a.b.c.d/nn`。无掩码（单 host）返回 None —— 那类走既有精确匹配表。
/// 拒绝 /0：写错一次等于封全站，宁要管理员显式逐段登记。
fn parse_netban(text: &str, reason: &str) -> Option<NetBan> {
    let (base, bits) = match text.split_once('/') {
        Some((b, n)) => (b, n.parse::<u32>().ok()?),
        None => return None,
    };
    let ip: IpAddr = base.trim_start_matches("::ffff:").parse().ok()?;
    match ip {
        IpAddr::V4(v4) => {
            if bits == 0 || bits > 32 {
                return None;
            }
            // 前缀在**高位**：/24 的掩码是 0xFFFFFF00，不是低 24 位
            let mask = if bits == 32 {
                !0u32
            } else {
                !0u32 << (32 - bits)
            };
            Some(NetBan::V4 {
                net: u32::from(v4) & mask,
                mask,
                reason: reason.to_string(),
            })
        }
        IpAddr::V6(v6) => {
            if bits == 0 || bits > 128 {
                return None;
            }
            let mask = if bits == 128 {
                u128::MAX
            } else {
                !0u128 << (128 - bits)
            };
            Some(NetBan::V6 {
                net: u128::from(v6) & mask,
                mask,
                reason: reason.to_string(),
            })
        }
    }
}

/// 把 `::ffff:a.b.c.d` 归一到 v4，避免同一个人跨族漏判。
fn fam(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => IpAddr::V6(v6),
        },
        other => other,
    }
}

fn hits(ban: &NetBan, ip: IpAddr) -> bool {
    match (ban, ip) {
        (NetBan::V4 { net, mask, .. }, IpAddr::V4(v4)) => {
            u32::from(v4) & mask == *net
        }
        (NetBan::V6 { net, mask, .. }, IpAddr::V6(v6)) => {
            u128::from(v6) & mask == *net
        }
        _ => false,
    }
}

/// 段封禁表（refresh_guard 每 60s 重建）
pub(crate) fn ban_nets() -> &'static RwLock<Vec<NetBan>> {
    static V: OnceLock<RwLock<Vec<NetBan>>> = OnceLock::new();
    V.get_or_init(|| RwLock::new(Vec::new()))
}

/// 把封禁行分流：带掩码的进段表，返回不进段表的那些（由调用方继续做精确匹配）。
/// 段格式无效（越界前缀、非 IP）只 warn 跳过——与 agent_rules 的失效可见性同题，
/// 下一批统一按「后台显示规则状态」收口。
pub(crate) fn load_bans(rows: &[(String, String)]) -> Vec<(String, String)> {
    let mut nets = Vec::new();
    let mut exact = Vec::new();
    for (text, reason) in rows {
        if !text.contains('/') {
            exact.push((text.clone(), reason.clone()));
            continue;
        }
        match parse_netban(text, reason) {
            Some(n) => nets.push(n),
            None => {
                tracing::warn!(%text, "ip_bans 段格式无效，规则跳过");
                exact.push((text.clone(), reason.clone()));
            }
        }
    }
    if let Ok(mut w) = ban_nets().write() {
        *w = nets;
    }
    exact
}

/// ip_bans 命中判定（精确表之外再查段表）。
pub(crate) fn ban_hit(ip: &str) -> Option<String> {
    let parsed = ip.trim().parse::<IpAddr>().ok().map(fam);
    let nets = match ban_nets().read() {
        Ok(r) => r,
        Err(e) => e.into_inner(),
    };
    if let Some(p) = parsed {
        for ban in nets.iter() {
            if hits(ban, p) {
                return match ban {
                    NetBan::V4 { reason, .. }
                    | NetBan::V6 { reason, .. } => Some(reason.clone()),
                };
            }
        }
    }
    None
}

/// 种子元数据：大小 / 完成数 / 发布者 / 审核态。键 = announce 侧原始 hex
/// （快照里同时以规范化 hash 与 raw hash 两个键写入）。
#[derive(Clone, Copy, Debug)]
pub(crate) struct SwarmMeta {
    pub(crate) size: i64,
    pub(crate) completed: i64,
    pub(crate) owner_id: i64,
    pub(crate) approval: i16,
}

pub(crate) fn meta_map(
) -> &'static RwLock<std::collections::HashMap<String, SwarmMeta>> {
    static V: OnceLock<
        RwLock<std::collections::HashMap<String, SwarmMeta>>,
    > = OnceLock::new();
    V.get_or_init(|| RwLock::new(std::collections::HashMap::new()))
}

pub(crate) fn meta_of(info_hash: &str) -> Option<SwarmMeta> {
    meta_map().read().ok()?.get(info_hash).copied()
}

/// 未注册 info_hash 负缓存（审计 10-07 P3）：旧版每次 miss 都直查 PG，
/// 随机 hash 洪水 = 一请求一查询，白名单自己成了 DB 放大器。
/// 60s 且每次 guard 刷新即清 ⇒ 新发种最迟一个刷新周期可 announce。
pub(crate) fn hash_miss(
) -> &'static RwLock<std::collections::HashSet<String>> {
    static V: OnceLock<RwLock<std::collections::HashSet<String>>> =
        OnceLock::new();
    V.get_or_init(|| RwLock::new(std::collections::HashSet::new()))
}

pub(crate) fn miss_seen(info_hash: &str) -> bool {
    match hash_miss().read() {
        Ok(r) => r.contains(info_hash),
        Err(e) => e.into_inner().contains(info_hash),
    }
}

pub(crate) fn remember_miss(info_hash: &str) {
    if let Ok(mut w) = hash_miss().write() {
        if w.len() > 200_000 {
            w.clear(); // 洪水兜底：宁可重新查一轮
        }
        w.insert(info_hash.to_string());
    }
}

pub(crate) fn clear_miss() {
    if let Ok(mut w) = hash_miss().write() {
        w.clear();
    }
}

/// scrape 侧限流档：与 announce 分桶（旧版共用 `rl:ann:ip:`，一次全站轮询
/// 就把该 IP 的 announce 额度吃光，用户表现为「突然全站在报频率超限」）。
pub(crate) fn scr_per_min() -> i64 {
    static V: OnceLock<i64> = OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("SCR_RATE_IP_PER_MIN")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(600)
            .max(1)
    })
}

/// 单次 scrape 最多接受多少个 info_hash（chihaya 50 / opentracker 64 口径）。
pub(crate) fn scrape_max_hashes() -> usize {
    static V: OnceLock<usize> = OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("SCR_MAX_HASHES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(64)
            .clamp(1, 256)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // 段封禁表是进程内静态，而 cargo 默认并行跑测试 ⇒ 碰它的用例必须串行，
    // 否则互相把对方的表冲掉（表现为随机红）。
    fn lock() -> std::sync::MutexGuard<'static, ()> {
        static L: std::sync::OnceLock<std::sync::Mutex<()>> =
            std::sync::OnceLock::new();
        L.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn load_fixture() -> usize {
        let exact = load_bans(&[
            ("10.9.8.0/24".into(), "机房段".into()),
            ("2001:db8:abcd::/48".into(), "教育网段".into()),
            ("8.8.8.8".into(), "单地址".into()),
            ("1.2.3.4/bogus".into(), "坏格式".into()),
            ("0.0.0.0/0".into(), "封全站".into()),
        ]);
        assert_eq!(exact.len(), 3, "单地址/坏格式留在精确表：{exact:?}");
        ban_nets().read().unwrap().len()
    }

    #[test]
    fn cidr_ban_matches_inside_and_not_outside() {
        let _g = lock();
        assert_eq!(load_fixture(), 2, "段表应正好收下两条合法段");
        assert!(ban_hit("10.9.8.77").is_some(), "段内地址应命中");
        assert!(ban_hit("10.9.9.1").is_none(), "段外不该命中");
        assert!(ban_hit("10.8.8.8").is_none());
        assert!(ban_hit("2001:db8:abcd:1::5").is_some(), "/48 内");
        assert!(ban_hit("2001:db8:abce::1").is_none(), "/48 外");
        assert!(ban_hit("not-an-ip").is_none(), "非法串不该炸");
    }

    #[test]
    fn v4_mapped_v6_matches_v4_prefix() {
        let _g = lock();
        let _ = load_fixture();
        assert!(
            ban_hit("::ffff:10.9.8.9").is_some(),
            "v4-mapped 应降级后按 v4 段匹配"
        );
    }

    #[test]
    fn whole_internet_ban_is_refused() {
        let _g = lock();
        let _ = load_bans(&[("0.0.0.0/0".into(), "封全站".into())]);
        assert!(ban_hit("1.1.1.1").is_none(), "/0 必须被拒绝");
    }

    #[test]
    fn miss_cache_roundtrip_and_clear() {
        let _g = lock();
        let h = "ff".repeat(20);
        assert!(!miss_seen(&h));
        remember_miss(&h);
        assert!(miss_seen(&h));
        clear_miss();
        assert!(!miss_seen(&h));
    }

    #[test]
    fn scrape_caps_are_bounded() {
        assert!((1..=256).contains(&scrape_max_hashes()));
        assert!(scr_per_min() >= 1);
    }
}
