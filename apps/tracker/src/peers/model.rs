//! Peer 数据模型：PeerKey / Peer / compact peer / 快照结构与回连常量。

#[derive(Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PeerKey {
    /// info_hash 的 hex 编码（40 字符），与 DB/事件流口径一致
    pub info_hash: String,
    /// peer_id 的 hex 编码
    pub peer_id: String,
}

/// 回连可达性（0071 connectable 抽样）：-1 未测 / 0 不可达 / 1 可达
pub const CONN_UNTESTED: i8 = -1;
pub const CONN_DEAD: i8 = 0;
pub const CONN_OK: i8 = 1;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)] // uploaded/downloaded/user_id 供后续审计扩展读取（all_unreachable 等）
pub struct Peer {
    pub key: PeerKey,
    pub ip: String,
    pub port: u16,
    pub uploaded: i64,
    pub downloaded: i64,
    pub left: i64,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub user_id: i64,
    pub connectable: i8,
}

impl Peer {
    pub fn is_seeder(&self) -> bool {
        self.left == 0
    }
}

#[derive(Clone, Copy)]
pub struct CompactPeer {
    pub ip: [u8; 4],
    pub port: u16,
}

/// BEP-7：IPv6 peer（16 字节 IP + 2 字节端口，进响应的 peers6 字段）。
/// 教育网（CERNET2）IPv6 覆盖率极高，纯 v6 用户拿不到 v4 peer —— 不实现 peers6 等于拒服务。
#[derive(Clone, Copy)]
pub struct CompactPeer6 {
    pub ip: [u8; 16],
    pub port: u16,
}

/// 同一 info_hash 的 peer 快照（v4/v6 分列，响应里分别进 peers / peers6）
#[derive(Default)]
pub struct Snapshot {
    pub v4: Vec<CompactPeer>,
    pub v6: Vec<CompactPeer6>,
}
