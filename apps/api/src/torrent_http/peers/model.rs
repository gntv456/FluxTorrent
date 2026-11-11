//! 在线 peer 的快照结构与纯展示函数。
//! 从 `peers.rs` 拆出：结构 / 格式化各自成域，主文件只留取数与接口，
//! 满足「单文件 ≤300 行」门禁（快照双形态回归单测撑大后外移）。

use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(super) struct SnapKey {
    pub(super) peer_id: String,
}

/// BEP 3/7 端点（与 `apps/tracker/src/peers/model.rs::Endpoint` 同形）
#[derive(Deserialize)]
pub(super) struct SnapEndpoint {
    pub(super) ip: String,
    pub(super) port: u16,
}

/// tracker 快照里的单个 peer。
///
/// 两个形态都得吃：`cfb4e32a`（peer 按地址族各存端点）之后写
/// `v4/v6`，之前写顶层 `ip/port`。只认旧形态会让**整份快照解析
/// 失败**，面板恒显示「当前没有在线用户」，而侧栏做种数照常有值
/// （2026-11-11 线上实测：7.7MB 快照、全部为 v4/v6 形态）。
#[derive(Deserialize)]
pub(super) struct SnapPeer {
    pub(super) key: SnapKey,
    #[serde(default)]
    pub(super) v4: Option<SnapEndpoint>,
    #[serde(default)]
    pub(super) v6: Option<SnapEndpoint>,
    /// 旧形态：升级前写出的历史行，容缺后由 TTL 自然淘汰
    #[serde(default)]
    pub(super) ip: Option<String>,
    #[serde(default)]
    pub(super) port: Option<u16>,
    pub(super) uploaded: i64,
    pub(super) downloaded: i64,
    pub(super) left: i64,
    pub(super) last_seen: chrono::DateTime<chrono::Utc>,
    pub(super) user_id: i64,
    pub(super) connectable: i8,
}

impl SnapPeer {
    /// 主端点：v4 优先，与 tracker `Peer::primary()` 同口径。
    /// 无端点的历史行回落空串/0，不让一行坏数据毁掉整份快照。
    pub(super) fn endpoint(&self) -> (&str, u16) {
        if let Some(ep) = self.v4.as_ref().or(self.v6.as_ref()) {
            return (ep.ip.as_str(), ep.port);
        }
        (self.ip.as_deref().unwrap_or(""), self.port.unwrap_or(0))
    }
}

#[derive(Serialize)]
pub(super) struct PeerItem {
    /// 谁在做种/下载（与 /snatches 同口径）
    pub(super) user_id: i64,
    pub(super) username: String,
    /// peer_id 前 8 字节（hex，16 字符）
    pub(super) peer_id: String,
    /// 客户端指纹（peer_id 前缀的 ASCII，如 `-qB4480-`）
    pub(super) client: String,
    pub(super) ip: String,
    pub(super) port: u16,
    pub(super) seeder: bool,
    /// 下载进度百分比（0–100）
    pub(super) progress: f64,
    pub(super) uploaded: i64,
    pub(super) downloaded: i64,
    /// 距最近一次 announce 的秒数
    pub(super) last_seen_secs: i64,
    /// 回连可达性：-1 未测 / 0 不可达 / 1 可达
    pub(super) connectable: i8,
    /// 是否当前登录者自己
    pub(super) is_self: bool,
}

/// 用户名回落：库里查不到（账号已删 / 查询失败）时不给空串，
/// 用 `#id` 让这一行仍可辨认，也不误报成「隐身用户」。
pub(super) fn display_name(
    names: &std::collections::HashMap<i64, String>,
    user_id: i64,
) -> String {
    names
        .get(&user_id)
        .cloned()
        .unwrap_or_else(|| format!("#{user_id}"))
}

/// peer_id（hex）→ 客户端指纹：前 8 字节的可打印 ASCII，不可打印补 '.'
pub(super) fn client_of(peer_id_hex: &str) -> String {
    let bytes = (0..peer_id_hex.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(peer_id_hex.get(i..i + 2)?, 16).ok())
        .take(8);
    bytes
        .map(|b| if b.is_ascii_graphic() { b as char } else { '.' })
        .collect()
}

/// IP 脱敏：v4 掩末段、v6 保留前 4 组；`full=true` 原样返回（staff）
pub(super) fn mask_ip(ip: &str, full: bool) -> String {
    if full {
        return ip.to_string();
    }
    if ip.contains(':') {
        let head: Vec<&str> = ip.split(':').take(4).collect();
        format!("{}::*", head.join(":"))
    } else {
        match ip.rsplit_once('.') {
            Some((head, _)) => format!("{head}.*"),
            None => ip.to_string(),
        }
    }
}

/// 进度百分比：已下载 / (已下载 + 剩余)；做种（left=0）恒 100
pub(super) fn progress_of(seeder: bool, downloaded: i64, left: i64) -> f64 {
    if seeder {
        return 100.0;
    }
    let total = downloaded.saturating_add(left);
    if total <= 0 {
        return 0.0;
    }
    ((downloaded as f64 / total as f64) * 1000.0).round() / 10.0
}
