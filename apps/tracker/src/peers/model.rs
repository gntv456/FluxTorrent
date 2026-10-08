//! Peer 数据模型：PeerKey / Peer / compact peer / 快照结构与回连常量。

#[derive(Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PeerKey {
    /// info_hash 的 hex 编码（40 字符），与 DB/事件流口径一致
    pub info_hash: String,
    /// peer_id 的 hex 编码
    pub peer_id: String,
}

/// 回连/协议实测结果（0071 connectable 抽样 + 2026-10-08 BT 三阶段探测）：
///   -1 UNTESTED —— 本轮未探测（未知，不作任何判定）
///    0 DEAD     —— **实测不可信**：端口不通 / 裸监听 / BT 握手应答但
///                 piece 哈希不符（实锤伪造数据）
///    1 OK       —— 实测可信：BT 握手 + bitfield + （抽样时）piece 哈希全过
///   -2 SUSPECT —— **无法验证**（2026-10-08 新增）：TCP 可连但不响应明文
///                 BT 协议。典型是「仅加密连接」客户端（qBittorrent 的
///                 only-encrypted / MSE-PE）或 peer 白名单限定。
///
///                 这一档存在的理由（通用 PT 站点尤其重要）：私有站客户端
///                 基线已知、可以要求用户关加密；通用站必须假设用户群里有
///                 各种客户端偏好，把它们一并判 DEAD 会误伤好用户、砍掉
///                 站点的做种供给。它与 DEAD 的区别必须在数据层保留——
///                 合并成一个值就再也分不出「骗钱的」和「用加密客户端的」。
///
///                 消费口径：只有 **DEAD(0)** 阻断收益/惩罚；
///                 SUSPECT(-2) 与 UNTESTED(-1) 同样放行，仅供观测。
pub const CONN_UNTESTED: i8 = -1;
pub const CONN_DEAD: i8 = 0;
pub const CONN_OK: i8 = 1;
pub const CONN_SUSPECT: i8 = -2;

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
    /// peer id 原始 20 字节（0267）：仅非 compact 响应需要；
    /// compact 响应不使用（BEP23 之后所有现代客户端走 compact），
    /// 存着是为了老客户端 `compact=0` 时能发出合规的 peer 字典。
    pub peer_id: [u8; 20],
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
