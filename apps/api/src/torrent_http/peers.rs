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
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let info_hash: Option<String> =
        sqlx::query_scalar("SELECT info_hash FROM torrents WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(ih) = info_hash else {
        // NotFound 的载荷是资源 id（errors.rs::NotFound(i64)）
        return Err(DomainError::NotFound(id));
    };

    // 快照缺失（tracker 未运行 / 刚重启 / 键过期）不算错误：返回 available=false
    let mut c = state.redis.clone();
    let raw: Option<String> = redis::AsyncCommands::get(&mut c, SNAPSHOT_KEY)
        .await
        .unwrap_or(None);
    let full_ip = auth.class_id >= 90;
    let now = chrono::Utc::now();
    let mut items: Vec<PeerItem> = Vec::new();
    let (mut seeders, mut leechers) = (0usize, 0usize);

    if let Some(raw) = raw {
        type Snap = Vec<(String, Vec<SnapPeer>)>;
        if let Ok(snap) = serde_json::from_str::<Snap>(&raw) {
            for (hash, peers) in snap {
                if hash != ih {
                    continue;
                }
                for p in peers {
                    let seeder = p.left == 0;
                    if seeder {
                        seeders += 1;
                    } else {
                        leechers += 1;
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
            }
        }
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
        "available": true,
        "masked": !full_ip,
        "seeders": seeders,
        "leechers": leechers,
        "total": total,
        "items": items,
    })))
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
