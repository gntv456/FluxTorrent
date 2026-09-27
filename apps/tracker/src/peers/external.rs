//! Peer 外置存储（0225 G30-B12：多 tracker 副本互见）。
//!
//! 现状缺口（附录 D 档三）：peer 表是进程内 DashMap + 每 60s 往同一个 Redis
//! 键全量覆盖写——多副本互不可见且互抹，是「tracker 多副本坏功能」的根。
//!
//! 改造：Redis Hash 按 swarm 分键（`flux:swarm:{info_hash}`，field=peer_id，
//! value=Peer JSON），所有副本读写同一份。TTL 取 SEEDER_TIMEOUT+冗余（断线
//! swarm 自然过期，与内存表 90s/3720s 分档淘汰语义对齐：读侧按 last_seen
//! 过滤，超时分档与内存一致）。
//!
//! 形态：FLUX_TRACKER_PEER_STORE=redis 启用；缺省空 = 内存单机（行为与
//! 以前完全一致，不引入 Redis 依赖路径的热路径开销）。

use redis::aio::ConnectionManager;
use redis::AsyncCommands;

use super::model::{
    CompactPeer, CompactPeer6, Peer, PeerKey, Snapshot, CONN_UNTESTED,
};
use super::table::{PEER_TIMEOUT, SEEDER_TIMEOUT};

/// 外置键：每个 swarm 一张 hash
fn swarm_key(info_hash: &str) -> String {
    format!("flux:swarm:{info_hash}")
}

/// 外置 TTL：做种分档上限 + 30s 冗余（读写两侧都有 last_seen 过滤，
/// TTL 只负责回收无人问津的 swarm 键）
fn swarm_ttl_secs() -> i64 {
    SEEDER_TIMEOUT.as_secs() as i64 + 30
}

/// 是否启用外置（进程级开关，env 读一次）
pub fn external_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("FLUX_TRACKER_PEER_STORE").unwrap_or_default() == "redis"
    })
}

fn alive(p: &Peer, now: &chrono::DateTime<chrono::Utc>) -> bool {
    let timeout = if p.left == 0 {
        SEEDER_TIMEOUT
    } else {
        PEER_TIMEOUT
    };
    now.signed_duration_since(p.last_seen)
        .to_std()
        .unwrap_or_default()
        < timeout
}

/// 写：upsert 单 peer（保留既有 connectable 测量值——与内存表同语义）
pub async fn upsert(
    redis: &mut ConnectionManager,
    peer: Peer,
) -> Result<(), redis::RedisError> {
    let key = swarm_key(&peer.key.info_hash);
    // connectable 保留：先读旧值
    let old: Option<String> = redis
        .hget(&key, &peer.key.peer_id)
        .await
        .unwrap_or(None);
    let mut p = peer;
    if let Some(old) = old {
        if let Ok(prev) = serde_json::from_str::<Peer>(&old) {
            p.connectable = prev.connectable;
        }
    }
    let val = serde_json::to_string(&p).unwrap_or_default();
    let _: () = redis.hset(&key, &p.key.peer_id, val).await?;
    let _: bool = redis.expire(&key, swarm_ttl_secs()).await?;
    Ok(())
}

/// 写：删除单 peer（stopped 事件）
pub async fn remove(
    redis: &mut ConnectionManager,
    key: &PeerKey,
) -> Result<(), redis::RedisError> {
    let _: () = redis.hdel(swarm_key(&key.info_hash), &key.peer_id).await?;
    Ok(())
}

/// 读：单 swarm 全量（读侧过滤超时，与内存表 gc_swarm 同口径）
async fn swarm_peers(
    redis: &mut ConnectionManager,
    info_hash: &str,
) -> Vec<Peer> {
    let all: std::collections::HashMap<String, String> = redis
        .hgetall(swarm_key(info_hash))
        .await
        .unwrap_or_default();
    let now = chrono::Utc::now();
    all.into_values()
        .filter_map(|v| serde_json::from_str::<Peer>(&v).ok())
        .filter(|p| alive(p, &now))
        .collect()
}

/// 读：peer 列表快照（v4/v6 分列、排除自己、numwant 上限——与内存表
/// snapshot 同语义；由调用方 clamp）
pub async fn snapshot(
    redis: &mut ConnectionManager,
    info_hash: &str,
    numwant: usize,
    exclude: &str,
) -> Snapshot {
    let mut snap = Snapshot::default();
    let peers = swarm_peers(redis, info_hash).await;
    for p in &peers {
        if snap.v4.len() + snap.v6.len() >= numwant {
            break;
        }
        if p.key.peer_id == exclude {
            continue;
        }
        let port = p.port;
        if let Ok(v6) = p.ip.parse::<std::net::Ipv6Addr>() {
            snap.v6.push(CompactPeer6 {
                ip: v6.octets(),
                port,
            });
        } else if let Ok(v4) = p.ip.parse::<std::net::Ipv4Addr>() {
            snap.v4.push(CompactPeer {
                ip: v4.octets(),
                port,
            });
        }
    }
    snap
}

/// 读：seeders/leechers 计数
pub async fn counts(
    redis: &mut ConnectionManager,
    info_hash: &str,
) -> (usize, usize) {
    let peers = swarm_peers(redis, info_hash).await;
    (
        peers.iter().filter(|p| p.is_seeder()).count(),
        peers.iter().filter(|p| !p.is_seeder()).count(),
    )
}

/// 读：回连状态（未命中 = 未测）
pub async fn connectable_of(
    redis: &mut ConnectionManager,
    key: &PeerKey,
) -> i8 {
    let v: Option<String> = redis
        .hget(swarm_key(&key.info_hash), &key.peer_id)
        .await
        .unwrap_or(None);
    v.and_then(|s| serde_json::from_str::<Peer>(&s).ok())
        .map(|p| p.connectable)
        .unwrap_or(CONN_UNTESTED)
}

/// 写：回连结果
pub async fn set_connectable(
    redis: &mut ConnectionManager,
    key: &PeerKey,
    reachable: bool,
) {
    let k = swarm_key(&key.info_hash);
    let v: Option<String> =
        redis.hget(&k, &key.peer_id).await.unwrap_or(None);
    if let Some(s) = v {
        if let Ok(mut p) = serde_json::from_str::<Peer>(&s) {
            p.connectable = if reachable { 1 } else { 0 };
            let val = serde_json::to_string(&p).unwrap_or_default();
            let _: () = redis.hset(&k, &key.peer_id, val).await.unwrap_or(());
        }
    }
}

/// 回连抽样候选（与内存表 sample_probes 同口径：未测优先、已测轮替）
pub async fn sample_probes(
    redis: &mut ConnectionManager,
    keys: &[PeerKey],
) -> Vec<(PeerKey, String, u16)> {
    // 调用方给候选键（内存表无外置时的路径不同）；此处按键批量查 connectable
    let mut out = Vec::with_capacity(keys.len());
    for key in keys {
        let v: Option<String> = redis
            .hget(swarm_key(&key.info_hash), &key.peer_id)
            .await
            .unwrap_or(None);
        if let Some(s) = v {
            if let Ok(p) = serde_json::from_str::<Peer>(&s) {
                out.push((key.clone(), p.ip.clone(), p.port));
            }
        }
    }
    out
}
