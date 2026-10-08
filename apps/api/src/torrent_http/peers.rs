//! 种子当前在线 peer（详情页「做种/下载者」面板）。
//!
//! 数据源：tracker 已把整个 swarm 表 JSON 快照周期性写入 Redis
//! （`flux:tracker:peers`，TTL 1800；见 `apps/tracker/src/main.rs`），
//! 故此处只需「读快照 + 按 info_hash 过滤」，**tracker 侧零改动**。
//!
//! 隐私口径（重要）：给非 staff 的响应一律脱敏——IP 掩掉末段、不返回用户名、
//! peer_id 只留前 8 字节（够识别客户端）。staff（class_id >= 90）才看完整 IP。
//! 站点没有"看谁在下载什么"的正当需求，默认就该看不见。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// tracker 写入的 swarm 快照键（写入方见 tracker main.rs，TTL 1800s）
const SNAPSHOT_KEY: &str = "flux:tracker:peers";
/// 外置 peer 存储（FLUX_TRACKER_PEER_STORE=redis）下的权威位置：
/// 每 swarm 一个 Hash，field=peer_id hex、value=Peer JSON。
const SWARM_KEY_PREFIX: &str = "flux:swarm:";

/// 多副本模式下 tracker 不再写单键快照（那正是副本互抹的根源），
/// 面板必须改读外置 Hash，否则永远显示「0 人在线」（审计 10-07 P1-6）。
fn peer_store_external() -> bool {
    std::env::var("FLUX_TRACKER_PEER_STORE").unwrap_or_default() == "redis"
}
/// 单次返回上限（大 swarm 只给最活跃的一批，计数仍按全量）
const MAX_ITEMS: usize = 50;

#[derive(Deserialize)]
struct SnapKey {
    peer_id: String,
}

/// tracker 快照里的单个 peer（字段名与 `apps/tracker/src/peers/model.rs` 对齐）
#[derive(Deserialize)]
struct SnapPeer {
    key: SnapKey,
    ip: String,
    port: u16,
    uploaded: i64,
    downloaded: i64,
    left: i64,
    last_seen: chrono::DateTime<chrono::Utc>,
    user_id: i64,
    connectable: i8,
}

#[derive(Serialize)]
struct PeerItem {
    /// peer_id 前 8 字节（hex，16 字符）
    peer_id: String,
    /// 客户端指纹（peer_id 前缀的 ASCII，如 `-qB4480-`）
    client: String,
    ip: String,
    port: u16,
    seeder: bool,
    /// 下载进度百分比（0–100）
    progress: f64,
    uploaded: i64,
    downloaded: i64,
    /// 距最近一次 announce 的秒数
    last_seen_secs: i64,
    /// 回连可达性：-1 未测 / 0 不可达 / 1 可达
    connectable: i8,
    /// 是否当前登录者自己
    is_self: bool,
}

/// peer_id（hex）→ 客户端指纹：前 8 字节的可打印 ASCII，不可打印补 '.'
fn client_of(peer_id_hex: &str) -> String {
    let bytes = (0..peer_id_hex.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(peer_id_hex.get(i..i + 2)?, 16).ok())
        .take(8);
    bytes
        .map(|b| if b.is_ascii_graphic() { b as char } else { '.' })
        .collect()
}

/// IP 脱敏：v4 掩末段、v6 保留前 4 组；`full=true` 原样返回（staff）
fn mask_ip(ip: &str, full: bool) -> String {
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
fn progress_of(seeder: bool, downloaded: i64, left: i64) -> f64 {
    if seeder {
        return 100.0;
    }
    let total = downloaded.saturating_add(left);
    if total <= 0 {
        return 0.0;
    }
    ((downloaded as f64 / total as f64) * 1000.0).round() / 10.0
}

#[get("/torrents/{id}/peers")]
async fn torrent_peers(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let id = path.into_inner();
    let auth = require_auth(&req, &state).await?;
    // 与主详情同口径（0286 / P1-1）：未过审种子的 info_hash 与在线 peer 不外泄
    crate::torrents::assert_visible(
        &state.repo.db,
        id,
        (auth.id, auth.class_id >= 90),
    )
    .await?;
    // 双口径（审计 10-07 P1-6）：tracker 的桶键是 announce 侧原始字节 hex
    // （= raw_info_hash），库内 info_hash 是规范化重编码口径。两者不同的种子
    // 旧实现恒显示 0 人在线——版主治理面看不见真实 peer。
    let hashes: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT info_hash, raw_info_hash FROM torrents WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((ih, ih_raw)) = hashes else {
        // NotFound 的载荷是资源 id（errors.rs::NotFound(i64)）
        return Err(DomainError::NotFound(id));
    };

    let mut c = state.redis.clone();
    let (available, peers) = load_peers(&mut c, &ih, ih_raw.as_deref()).await;
    let full_ip = auth.class_id >= 90;
    let now = chrono::Utc::now();
    let mut items: Vec<PeerItem> = Vec::new();
    // 实时计数与 tracker 的 announce/scrape 口径对齐（P2-6）：同一账号在一个
    // swarm 里只计一次，否则多开客户端的人把自己显示成 10 个做种者
    let (mut seeders, mut leechers) = (
        std::collections::HashSet::<i64>::new(),
        std::collections::HashSet::<i64>::new(),
    );
    for p in peers {
        let seeder = p.left == 0;
        if seeder {
            seeders.insert(p.user_id);
        } else {
            leechers.insert(p.user_id);
        }
        items.push(PeerItem {
            peer_id: p.key.peer_id.chars().take(16).collect(),
            client: client_of(&p.key.peer_id),
            ip: mask_ip(&p.ip, full_ip),
            port: p.port,
            seeder,
            progress: progress_of(seeder, p.downloaded, p.left),
            uploaded: p.uploaded,
            downloaded: p.downloaded,
            last_seen_secs: now
                .signed_duration_since(p.last_seen)
                .num_seconds()
                .max(0),
            connectable: p.connectable,
            is_self: p.user_id == auth.id,
        });
    }
    // 做种者优先，其次最近活跃
    items.sort_by(|a, b| {
        b.seeder
            .cmp(&a.seeder)
            .then(a.last_seen_secs.cmp(&b.last_seen_secs))
    });
    let total = items.len();
    items.truncate(MAX_ITEMS);

    Ok(ok(serde_json::json!({
        // available 如实反映数据源是否存在：旧版恒 true，前端无法区分
        // 「tracker 挂了」与「真没人」（代码注释本来就说要返回 false）
        "available": available,
        "masked": !full_ip,
        "seeders": seeders.len(),
        "leechers": leechers.len(),
        "total": total,
        "items": items,
    })))
}

/// 取该种子的在线 peer：外置模式读 `flux:swarm:{hash}`，单机模式读周期快照。
/// 返回 (数据源是否存在, peers)。
async fn load_peers(
    c: &mut redis::aio::ConnectionManager,
    ih: &str,
    ih_raw: Option<&str>,
) -> (bool, Vec<SnapPeer>) {
    use redis::AsyncCommands;
    if peer_store_external() {
        let key = format!("{SWARM_KEY_PREFIX}{}", ih_raw.unwrap_or(ih));
        let exists: i64 = redis::cmd("EXISTS")
            .arg(&key)
            .query_async(c)
            .await
            .unwrap_or(0);
        let map: std::collections::HashMap<String, String> =
            redis::AsyncCommands::hgetall(c, key)
                .await
                .unwrap_or_default();
        let peers = map
            .values()
            .filter_map(|v| serde_json::from_str::<SnapPeer>(v).ok())
            .collect();
        return (exists > 0, peers);
    }
    let raw: Option<String> = redis::AsyncCommands::get(c, SNAPSHOT_KEY)
        .await
        .unwrap_or(None);
    let available = raw.is_some();
    type Snap = Vec<(String, Vec<SnapPeer>)>;
    let peers = raw
        .and_then(|r| serde_json::from_str::<Snap>(&r).ok())
        .map(|snap| {
            snap.into_iter()
                .filter(|(h, _)| h == ih || ih_raw.is_some_and(|r| h == r))
                .flat_map(|(_, ps)| ps)
                .collect()
        })
        .unwrap_or_default();
    (available, peers)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 客户端指纹只取可打印 ASCII，不可打印补点（防注入进 HTML）
    #[test]
    fn client_fingerprint_is_printable() {
        // "-qB4480-" 的 hex（qBittorrent 4.4.80）
        let hex = "2d7142343438302d";
        assert_eq!(client_of(hex), "-qB4480-");
        // 含不可打印字节（\u{1}）时补 '.'
        assert_eq!(client_of("01414243"), ".ABC");
        // 奇数长度/非法 hex 不 panic，能解多少算多少
        assert_eq!(client_of("2d7"), "-");
        assert_eq!(client_of(""), "");
    }

    /// 非 staff 必须脱敏：v4 掩末段、v6 保留前 4 组；staff 看完整
    #[test]
    fn mask_ip_hides_host_part() {
        assert_eq!(mask_ip("1.2.3.4", false), "1.2.3.*");
        assert_eq!(mask_ip("1.2.3.4", true), "1.2.3.4");
        assert_eq!(
            mask_ip("2001:db8:85a3:8d3:1319:8a2e:370:7348", false),
            "2001:db8:85a3:8d3::*"
        );
        assert_eq!(mask_ip("::1", false), "::1::*");
        // 非法输入不 panic
        assert_eq!(mask_ip("notanip", false), "notanip");
    }

    /// 进度：做种恒 100；下载中按比例；零总量不除零
    #[test]
    fn progress_edges() {
        assert_eq!(progress_of(true, 0, 0), 100.0);
        assert_eq!(progress_of(false, 0, 0), 0.0);
        assert_eq!(progress_of(false, 50, 50), 50.0);
        assert_eq!(progress_of(false, 1, 2), 33.3);
    }
}
