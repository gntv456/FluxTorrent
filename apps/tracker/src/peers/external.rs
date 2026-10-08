//! Peer 外置存储（0225 G30-B12：多 tracker 副本互见）。
//!
//! 现状缺口（附录 D 档三）：peer 表是进程内 DashMap + 每 60s 往同一个 Redis
//! 键全量覆盖写——多副本互不可见且互抹，是「tracker 多副本坏功能」的根。
//!
//! 改造：Redis Hash 按 swarm 分键（`flux:swarm:{info_hash}`，field=peer_id，
//! value=Peer JSON），所有副本读写同一份。TTL 取做种分档+冗余（断线
//! swarm 自然过期，与内存表分档淘汰语义对齐：读侧按 last_seen
//! 过滤，超时分档与内存一致）。
//!
//! 形态：FLUX_TRACKER_PEER_STORE=redis 启用；缺省空 = 内存单机（行为与
//! 以前完全一致，不引入 Redis 依赖路径的热路径开销）。

use redis::aio::ConnectionManager;
use redis::AsyncCommands;

use super::model::{
    CompactPeer, CompactPeer6, Peer, PeerKey, Snapshot, CONN_UNTESTED,
};
use super::ttl_for;

/// 外置键：每个 swarm 一张 hash
fn swarm_key(info_hash: &str) -> String {
    format!("flux:swarm:{info_hash}")
}

/// 外置 TTL：做种分档上限 + 30s 冗余（读写两侧都有 last_seen 过滤，
/// TTL 只负责回收无人问津的 swarm 键）
fn swarm_ttl_secs() -> i64 {
    ttl_for(0).as_secs() as i64 + 30
}

/// 是否启用外置（进程级开关，env 读一次）
pub fn external_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("FLUX_TRACKER_PEER_STORE").unwrap_or_default() == "redis"
    })
}

fn alive(p: &Peer, now: &chrono::DateTime<chrono::Utc>) -> bool {
    now.signed_duration_since(p.last_seen)
        .to_std()
        .unwrap_or_default()
        < ttl_for(p.left)
}

/// 写：upsert 单 peer（保留既有 connectable 测量值——与内存表同语义）
pub async fn upsert(
    redis: &mut ConnectionManager,
    peer: Peer,
) -> Result<(), redis::RedisError> {
    // 审查修正（P1-1）：HGET→改→HSET 读改写竞态会吞回连测量值（upsert
    // 覆盖 connectable 回 -1）。Lua 原子化：旧值存在则保留其 connectable。
    let key = swarm_key(&peer.key.info_hash);
    let new_val = serde_json::to_string(&peer).unwrap_or_default();
    let script = redis::Script::new(
        r#"local old = redis.call('HGET', KEYS[1], ARGV[1])
           if old then
             local ok, o = pcall(cjson.decode, old)
             -- 归属校验（审计 10-07 P1-1，与内存表 upsert 同语义）：槽位属于
             -- 别人就原样返回，不改写 ip/port/left，也不把可达测量传给顶号者
             if ok and o.user_id ~= nil and tostring(o.user_id) ~= ARGV[4] then
               return 2
             end
             if ok and o.connectable ~= nil then
               local n = cjson.decode(ARGV[2])
               n.connectable = o.connectable
               redis.call('HSET', KEYS[1], ARGV[1], cjson.encode(n))
               redis.call('EXPIRE', KEYS[1], ARGV[3])
               return 1
             end
           end
           redis.call('HSET', KEYS[1], ARGV[1], ARGV[2])
           redis.call('EXPIRE', KEYS[1], ARGV[3])
           return 0"#,
    );
    let mut inv = script.prepare_invoke();
    inv.key(key)
        .arg(&peer.key.peer_id)
        .arg(new_val)
        .arg(swarm_ttl_secs())
        .arg(peer.user_id);
    let _: i32 = inv.invoke_async(redis).await?;
    Ok(())
}

/// 写：删除单 peer（stopped 事件）。归属校验在 Lua 内原子完成（审计 10-06
/// 第 2 条）：peer_id 客户端自报，旧实现任意账号可用他人 peer_id 把对方
/// 从 swarm 踢下线。user_id 不匹配则不动。
pub async fn remove(
    redis: &mut ConnectionManager,
    key: &PeerKey,
    user_id: i64,
) -> Result<(), redis::RedisError> {
    let script = redis::Script::new(
        r#"local old = redis.call('HGET', KEYS[1], ARGV[1])
           if old then
             local ok, o = pcall(cjson.decode, old)
             if ok and o.user_id ~= nil then
               if tostring(o.user_id) ~= ARGV[2] then return 0 end
             end
           end
           redis.call('HDEL', KEYS[1], ARGV[1])
           return 1"#,
    );
    let mut inv = script.prepare_invoke();
    inv.key(swarm_key(&key.info_hash))
        .arg(&key.peer_id)
        .arg(user_id);
    let _: i32 = inv.invoke_async(redis).await?;
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
    self_user: i64,
) -> Snapshot {
    let mut snap = Snapshot::default();
    let peers = swarm_peers(redis, info_hash).await;
    for p in &peers {
        if snap.v4.len() + snap.v6.len() >= numwant {
            break;
        }
        // 与内存表同口径：请求者同账号的 peer 一律不下发（P3-6）
        if p.user_id == self_user {
            continue;
        }
        // 与内存表 snapshot 同口径（审计 10-06 第 5 条）：port=0 不可连接
        if p.port == 0 {
            continue;
        }
        let port = p.port;
        let peer_id = crate::peers::peer_id_bytes(&p.key.peer_id);
        if let Ok(v6) = p.ip.parse::<std::net::Ipv6Addr>() {
            snap.v6.push(CompactPeer6 {
                ip: v6.octets(),
                port,
            });
        } else if let Ok(v4) = p.ip.parse::<std::net::Ipv4Addr>() {
            snap.v4.push(CompactPeer {
                ip: v4.octets(),
                port,
                peer_id,
            });
        }
    }
    snap
}

/// 读：seeders/leechers 计数（与内存表口径一致：port=0 不计，
/// 且按 user_id 去重——peer_id 客户端自报，同账号多 peer_id 不应
/// 把实时在线数灌成倍数，见 table.rs::count_seeders P2-6 注释）
pub async fn counts(
    redis: &mut ConnectionManager,
    info_hash: &str,
) -> (usize, usize) {
    let peers = swarm_peers(redis, info_hash).await;
    let mut seeders = std::collections::HashSet::new();
    let mut leechers = std::collections::HashSet::new();
    for p in peers.iter().filter(|p| p.port != 0) {
        if p.is_seeder() {
            seeders.insert(p.user_id);
        } else {
            leechers.insert(p.user_id);
        }
    }
    (seeders.len(), leechers.len())
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
    let v: Option<String> = redis.hget(&k, &key.peer_id).await.unwrap_or(None);
    if let Some(s) = v {
        if let Ok(mut p) = serde_json::from_str::<Peer>(&s) {
            p.connectable = if reachable { 1 } else { 0 };
            let val = serde_json::to_string(&p).unwrap_or_default();
            let _: () = redis.hset(&k, &key.peer_id, val).await.unwrap_or(());
        }
    }
}

/// 回连探测：按调用方给的候选键查外置 peer（补全 ip/port）。
/// 未测优先/轮替的筛选在内存表 sample_probes（候选生成在内存侧），
/// 本函数只负责按键补全——与内存表口径并非同构。
pub async fn sample_probes(
    redis: &mut ConnectionManager,
    keys: &[PeerKey],
) -> Vec<(PeerKey, String, u16)> {
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
