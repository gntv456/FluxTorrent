//! BEP15 包常量、passkey 提取与 connection_id PRNG。

use std::time::Duration;

/// BEP-15 connect 魔数 0x41727101980。修正前误写为 0x41727109807a——
/// 所有标准客户端的 UDP connect 全部被静默丢弃，UDP tier 自上线起不可用
/// （默认不下发 UDP tier 故未暴露；0230 G31-E 实测抓到）。
pub(super) const PROTOCOL_ID: u64 = 0x0000_0417_2710_1980;
pub(super) const CONNECT_ACTION: u32 = 0;
pub(super) const ANNOUNCE_ACTION: u32 = 1;
pub(super) const SCRAPE_ACTION: u32 = 2;
pub(super) const ERROR_ACTION: u32 = 3;
/// connection_id 有效窗口（BEP15 推荐 60s；客户端实现普遍到点自动重连）
pub(super) const CONN_TTL: Duration = Duration::from_secs(60);
/// 会话表上限：防伪造 connect 洪水撑爆内存（LRU 语义：满了整体清，正常量级远达不到）
pub(super) const CONN_CAP: usize = 100_000;

/// 从 announce URL 提取 passkey 的客户端兼容集：
/// ① tracker_id 字段（qbittorrent/Transmission 传 path 段）；
/// ② 部分客户端（libtorrent rasterbar）把 URL query 塞进 tracker_id，
///    形如 "passkey=xxxx"——两种都识别。
pub(super) fn passkey_from_tracker_id(s: &str) -> &str {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("passkey=") {
        return rest;
    }
    s
}

/// connection_id 生成：RandomState（std 内置 SipHash，密钥源自系统随机）做 PRF，
/// 混入全局计数器与纳秒时钟——不可预测且不重复。旧实现 xorshift64* 固定种子
/// 0x9E3779B97F4A7C15：算法可逆，观测若干 connection_id 即可推演出后续全部
/// 序列，攻击者无需先 connect 就能伪造任意 (ip,port) 的 announce（审计 10-06
/// 第 8 条）。Cargo 无 rand 依赖，RandomState 是标准库内唯一带随机密钥的哈希。
pub(super) fn rand_u64() -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static STATE: std::sync::OnceLock<RandomState> = std::sync::OnceLock::new();
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let rs = STATE.get_or_init(RandomState::new);
    let mut h = rs.build_hasher();
    h.write_u64(SEQ.fetch_add(1, Ordering::Relaxed));
    h.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0),
    );
    h.finish()
}
