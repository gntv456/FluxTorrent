//! announce 的两条「按站型可调」的口径（审计 2026-10-08 五轮自 gate.rs 拆出，
//! 300 行门禁）：端口黑名单 + interval 抖动。
//!
//! 两者都是**策略面**而非主链路：判式短、env 一次性读取、可单测，
//! 放一起比塞进准入判定与响应编码各自的文件更好读。

use super::helpers::TrackerState;

/// 端口黑名单（PT 惯例：UNIT3D `AnnounceController::BLACK_PORTS`、
/// NexusPHP `portblacklisted()`）。五轮补上的是上一轮报告承诺过、当时只做
/// 完一半的那半—— `< 1024` 特权端口挡了，但**服务端口/已知工具端口**照收：
///   · 它们会被下发给同 swarm 的其他客户端 ⇒ 别人的客户端去连受害者的
///     445/3389/6379（连接面投毒 + 端口探测）；
///   · 它们同时是 tracker 自己回连抽样的目标 ⇒ 私有 tracker 变成内网端口
///     扫描器（10-07 实测过 tracker 真连上了内网 5432；IP 那半边堵住了，
///     端口这半边当时还开着）。
/// 默认表 = UNIT3D 那份（1214/3389/4662/6346/6347/6699/8080/8081）
/// 加上本站作为「反连目标」最危险的常见服务口。
///
/// **刻意不照抄 NexusPHP 的 6881–6889**：那一段是 m-t 时代的假客户端/爬虫常用段，
/// 但今天 qBittorrent/Transmission 的默认监听口正是它——照抄会把一大半
/// 真做种者判成非法端口。建站口径下这种「按某一家历史包袱收口」的选择
/// 留给站长：`ANN_BLACK_PORTS` 里自行追加即可。
/// `ANN_BLACK_PORTS` 显式置空 = 整条关闭（只保留 <1024 那一档）。
pub(crate) fn black_ports() -> &'static std::collections::BTreeSet<u16> {
    static V: std::sync::OnceLock<std::collections::BTreeSet<u16>> =
        std::sync::OnceLock::new();
    V.get_or_init(|| {
        // <1024 是另一条独立判据（见 gate::announce_gate），这里只列服务/工具端口
        const DEFAULT: [u16; 22] = [
            25,    // SMTP
            110,   // POP3
            135,   // MSRPC
            139,   // NetBIOS
            445,   // SMB
            1214,  // eDonkey（UNIT3D 表）
            1433,  // MSSQL
            2049,  // NFS
            3306,  // MySQL
            3389,  // RDP
            4662,  // eMule（UNIT3D 表）
            5432,  // PostgreSQL
            5900,  // VNC
            5984,  // CouchDB
            6346,  // gnutella-skew（UNIT3D 表）
            6347,  // gnutella（UNIT3D 表）
            6379,  // Redis
            6699,  // Azureus（UNIT3D 表）
            8080,  // HTTP-alt（UNIT3D 表）
            8081,  // HTTP-alt（UNIT3D 表）
            9200,  // Elasticsearch
            11211, // Memcached
        ];
        let raw = std::env::var("ANN_BLACK_PORTS");
        match raw {
            // 显式空串 = 站长主动关掉黑名单
            Ok(s) if s.trim().is_empty() => std::collections::BTreeSet::new(),
            Ok(s) => s
                .split(',')
                .filter_map(|p| p.trim().parse::<u16>().ok())
                .collect(),
            Err(_) => DEFAULT.into_iter().collect(),
        }
    })
}

/// 该端口是否命中黑名单。`port=0` 不在此列——它表示「我没开监听」，
/// 走的是「不计数不下发」那条独立判据，不是违规。
pub(crate) fn port_blacklisted(port: u16) -> bool {
    port != 0 && black_ports().contains(&port)
}

/// interval 抖动比例（`ANN_INTERVAL_JITTER_PCT`，默认 ±10%）。
pub(crate) fn jitter_pct() -> i64 {
    static V: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        super::helpers::env_i64("ANN_INTERVAL_JITTER_PCT", 10).clamp(0, 25)
    })
}

/// 在全站 interval 上加**稳定**抖动（chihaya 专门有 `interval_variation`
/// 中间件，UNIT3D 也在 announce 里随机化 interval）。
///
/// 全站固定 1800s 的后果是惊群：全体客户端在同一秒重发 announce，
/// 峰值正好压在限流窗与 PG 上。抖动按 `(info_hash, user_id)` 取确定值——
/// 同一客户端拿到的 interval 不会每次跳变（客户端按它排程，值跳变等于
/// 自己制造重发），不同 peer 之间又彼此错开。
pub(crate) fn jittered_interval(
    interval: i64,
    info_hash: &str,
    user: i64,
) -> i64 {
    let pct = jitter_pct();
    if pct <= 0 || interval <= 0 {
        return interval;
    }
    let span = interval.saturating_mul(pct) / 100;
    if span <= 0 {
        return interval;
    }
    // FNV-1a（与 emit.rs 的 agent 去重键同一套散列）
    let mut h: u64 = 0xcbf29ce484222325;
    for b in info_hash.as_bytes() {
        h = (h ^ (*b as u64)).wrapping_mul(0x100000001b3);
    }
    h = (h ^ user as u64).wrapping_mul(0x100000001b3);
    let width = (span as u64).saturating_mul(2).saturating_add(1);
    let off = (h % width) as i64 - span;
    interval.saturating_add(off).max(30)
}

/// 本站下发给这个 (账号, 种子) 的间隔。**唯一口径**：响应里写的值与
/// 任何「按间隔说话」的判据都必须走这里，否则等于「告诉客户端 1710s、
/// 却按 1800s 判它」。
pub(crate) fn peer_interval(
    state: &TrackerState,
    info_hash: &str,
    user: i64,
) -> i64 {
    let (base, _min) = state.intervals();
    jittered_interval(base, info_hash, user)
}

#[cfg(test)]
mod tests {
    use super::{jittered_interval, port_blacklisted};

    #[test]
    fn jitter_is_stable_bounded_and_spreads_peers() {
        let ih = "aa".repeat(20);
        // 同一 (种子, 账号) 必须恒定：客户端按下发的 interval 排程，
        // 值每次跳动等于自己制造重发
        assert_eq!(
            jittered_interval(1800, &ih, 7),
            jittered_interval(1800, &ih, 7)
        );
        // ±10% 之内
        let v = jittered_interval(1800, &ih, 7);
        assert!((1620..=1980).contains(&v), "抖动越界：{v}");
        // 换账号也要各自稳定
        assert_eq!(
            jittered_interval(1800, &ih, 8),
            jittered_interval(1800, &ih, 8)
        );
        // 一批不同种子必须真的被错开，否则等于没抖
        let spread: std::collections::HashSet<i64> = (0..60)
            .map(|i| jittered_interval(1800, &format!("{i:040x}"), 7))
            .collect();
        assert!(
            spread.len() > 10,
            "peer 没被错开，只有 {} 个取值",
            spread.len()
        );
        // 脏输入不 panic、不把 interval 变成 0
        assert_eq!(jittered_interval(-10, &ih, 7), -10);
        assert_eq!(jittered_interval(0, &ih, 7), 0);
    }

    #[test]
    fn service_ports_are_blacklisted_but_bt_ports_survive() {
        // env 未设时才是内置默认表；站长用 ANN_BLACK_PORTS 覆盖过的话
        // 这条用例的前提不成立（不该把配置态当代码态断言）
        if std::env::var("ANN_BLACK_PORTS").is_ok() {
            return;
        }
        for p in [
            25u16, 110, 135, 139, 445, 1214, 1433, 3306, 3389, 4662, 5432,
            6346, 6347, 6379, 6699, 8080, 8081,
        ] {
            assert!(
                port_blacklisted(p),
                "{p} 是服务/工具端口，不该被当 BT 监听口"
            );
        }
        // 真实客户端监听段：误杀一个就是全站一半人被判「无法做种」。
        // 6881-6889 是 qBittorrent/Transmission 的默认段，NexusPHP 把它列进
        // 黑名单是 m-t 时代的历史包袱，本站刻意不跟。
        for p in [1080u16, 6881, 6884, 6889, 51413, 49152, 65534, 65535] {
            assert!(!port_blacklisted(p), "{p} 是正常监听口，不该被拒");
        }
        // port=0 = 「我没开监听」，走另一条判据（不计数不下发），不是违规
        assert!(!port_blacklisted(0));
    }
}
