//! 内存 Peer 表（§5.2 tracker 职责：按 info_hash 分桶 + 桶内 HashMap，90s 超时淘汰）。
//!
//! 按域拆分：model（Peer/快照数据模型）、table（PeerTable 分桶与 GC）、
//! external（0225 G30-B12 Redis 外置存储——多 tracker 副本互见）、
//! bencode（BEP3/BEP-7 编码与字节工具）。

mod bencode;
pub mod external;
mod model;
mod table;

pub use bencode::{
    bencode_announce, bencode_scrape, hex, peer_id_bytes, percent_decode,
};
pub use model::{Peer, PeerKey, CONN_UNTESTED};
// 以下仅在 peers 内部与测试使用（bin crate 私有模块的 re-export 未用时告警）
#[cfg(test)]
pub use model::{CompactPeer, CompactPeer6};
pub use table::PeerTable;

/// 存活 TTL 基准：随 site_settings.announce_interval 伸缩（guard 刷新时写入）。
/// 旧实现硬编码 90s/3720s——interval=1800s 下 leecher 在两次 announce 之间
/// （90s 后）就从内存表除名，在线计数长期偏低（审计 10-06 第 4 条）。
static INTERVAL_SECS: std::sync::atomic::AtomicI64 =
    std::sync::atomic::AtomicI64::new(1800);

pub(crate) fn set_interval_secs(v: i64) {
    use std::sync::atomic::Ordering;
    INTERVAL_SECS.store(v.clamp(60, 86400), Ordering::Relaxed);
}

/// 分档存活 TTL：做种 2×interval+120（挂种客户端的汇报周期从快照/ttl 语义
/// 自然除名），传输中 interval+120（至少覆盖一个完整汇报周期）。
pub(crate) fn ttl_for(left: i64) -> std::time::Duration {
    use std::sync::atomic::Ordering;
    let iv = INTERVAL_SECS.load(Ordering::Relaxed).max(60) as u64;
    if left == 0 {
        std::time::Duration::from_secs(2 * iv + 120)
    } else {
        std::time::Duration::from_secs(iv + 120)
    }
}

#[cfg(test)]
mod tests;
