//! 内存 Peer 表（§5.2 tracker 职责：按 info_hash 分桶 + 桶内 HashMap，90s 超时淘汰）。
//!
//! 按域拆分：model（Peer/快照数据模型）、table（PeerTable 分桶与 GC）、
//! bencode（BEP3/BEP-7 编码与字节工具）。

mod bencode;
mod model;
mod table;

pub use bencode::{bencode_announce, bencode_scrape, hex, percent_decode};
pub use model::{Peer, PeerKey, CONN_UNTESTED};
// 以下仅在 peers 内部与测试使用（bin crate 私有模块的 re-export 未用时告警）
#[cfg(test)]
pub use model::{CompactPeer, CompactPeer6};
pub use table::PeerTable;

#[cfg(test)]
mod tests;
