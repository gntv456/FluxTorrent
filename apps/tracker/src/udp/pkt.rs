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

/// 轻量 PRNG（xorshift64*）：connection_id 只需不可预测，无需密码学强度
pub(super) fn rand_u64() -> u64 {
    use std::cell::Cell;
    thread_local! {
        static SEED: Cell<u64> = const { Cell::new(0x9E3779B97F4A7C15) };
    }
    SEED.with(|s| {
        let mut x = s.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        x.wrapping_mul(0x2545F4914F6CDD1D)
    })
}
