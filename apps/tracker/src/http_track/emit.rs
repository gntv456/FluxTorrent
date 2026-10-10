//! bencode 错误 + agent 块投递 + XADD。
//! 从 tracker main.rs 按域拆出。

use crate::peers;
use actix_web::HttpResponse;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// 计费事件投递失败后的进程内重投队列：`(stream, payload)` 先进先出。
///
/// 为什么不给客户端回 `failure reason` 让它重发：`completed` 事件客户端只发
/// 一次，报错并不保证重发，snatch 完成态与 H&R 记录会永久丢；而 up/down 是
/// 累计值，重投成功的第二条被 worker 的 anchor 判成零增量。队列两头都不丢。
const SPILL_MAX: usize = 20_000;
/// 单轮顺带排空的上限（Redis 刚恢复时不一次灌满）
const SPILL_DRAIN_BATCH: usize = 500;

static SPILL: OnceLock<Mutex<VecDeque<(String, serde_json::Value)>>> =
    OnceLock::new();
/// 队列深度与累计丢弃数（/metrics 暴露，见 http_track::metrics）
pub(crate) static SPILL_DEPTH: AtomicU64 = AtomicU64::new(0);
pub(crate) static SPILL_DROPPED: AtomicU64 = AtomicU64::new(0);

fn spill() -> &'static Mutex<VecDeque<(String, serde_json::Value)>> {
    SPILL.get_or_init(Default::default)
}

pub(crate) async fn emit_event(
    redis: &redis::aio::ConnectionManager,
    info_hash_hex: &str,
    user: i64,
    up: i64,
    down: i64,
    event: &str,
    left: i64,
    ip: &str,
    conn: i8,
    agent: &str,
    port: u16,
) {
    let mut payload = serde_json::json!({
        "user": user, "hash": info_hash_hex, "up": up, "down": down,
        "event": event, "left": left,
        // P0-2（2026-10-07 保种组审计）：上报端口进事件流——port=0 的
        // 「幽灵做种」在计费侧不得按在种累计（无监听端口不可能提供上传）。
        "port": port,
        "ts": chrono::Utc::now().to_rfc3339(),
    });
    // 0071：仅在有测量值时携带（worker 端 Option 语义：缺省 = 不覆盖 snatches.connectable）
    if !ip.is_empty() {
        payload["ip"] = serde_json::json!(ip);
    }
    if conn != peers::CONN_UNTESTED {
        payload["conn"] = serde_json::json!(conn);
    }
    // 0098：BT 客户端 UA（下载列表「客户端」列 + 反作弊证据）；截断防事件膨胀。
    // ZT81（2026-10-02）：按**字符边界**截断——原实现 `&agent[..len]` 按字节切片，
    // UA 含多字节 UTF-8 且恰好落在第 200 字节的非边界时会 panic（该 announce 500）。
    if !agent.is_empty() {
        let ua: String = agent.chars().take(200).collect();
        payload["agent"] = serde_json::json!(ua);
    }
    xadd_durable(redis, "flux:announce", &payload).await;
}

/// 交叉上报投递（2026-10-07 P0-2 治本）：把 leecher 声明的
/// 「从 peer X 下载了 N 字节」推入独立流 flux:xreport，供 worker
/// 佐证上传者的上传量。
///
/// 载荷：reports = [[上传者 user_id, 字节数], ...]（已在 tracker 侧
/// 校验过：目标是本 swarm 存活做种 peer、且非自己）。ts 取事件时点，
/// worker 按此裁决促销/时效，与 announce 事件同口径。
pub(crate) async fn emit_xreport(
    redis: &redis::aio::ConnectionManager,
    info_hash_hex: &str,
    leecher: i64,
    reports: &[(i64, i64)],
    ip: &str,
) {
    let payload = serde_json::json!({
        "hash": info_hash_hex,
        "leecher": leecher,
        "reports": reports,
        "ip": ip,
        "ts": chrono::Utc::now().to_rfc3339(),
    });
    // 佐证量也是账：漏一条 = 上传者少一笔可信上传，走 durable 通道
    xadd_durable(redis, "flux:xreport", &payload).await;
}

/// agent_rules 命中投递：同 (user, agent) 1 小时去重（SET NX EX），命中才 XADD flux:agent_block。
/// worker 消费落 cheat_events（hits 累加 / 首次进管理组信箱）—— 高频拒绝路径零 DB 开销。
pub(crate) async fn emit_agent_block(
    redis: &redis::aio::ConnectionManager,
    user: i64,
    agent: &str,
    ip: &str,
    reason: &str,
) {
    use redis::AsyncCommands;
    let mut c = redis.clone();
    // agent 任意字节字符串 → 稳定短键（FNV-1a，仅作去重键）
    let mut h: u64 = 0xcbf29ce484222325;
    for b in agent.as_bytes() {
        h = (h ^ (*b as u64)).wrapping_mul(0x100000001b3);
    }
    let dedup = format!("flux:agentblock:{user}:{h:016x}");
    let fresh: Option<bool> = c.set_nx(&dedup, 1).await.ok();
    let _: Result<(), _> = c.expire(&dedup, 3600).await;
    match fresh {
        Some(true) => {
            let payload = serde_json::json!({
                "user": user, "agent": agent, "ip": ip, "reason": reason,
                "ts": chrono::Utc::now().to_rfc3339(),
            });
            xadd(redis, "flux:agent_block", &payload).await;
        }
        _ => {} // Redis 故障或 1h 内重复命中：静默丢弃（拒绝本身不受影响）
    }
}

pub(crate) async fn xadd(
    redis: &redis::aio::ConnectionManager,
    stream: &str,
    payload: &serde_json::Value,
) {
    if let Err(e) = xadd_once(redis, stream, payload).await {
        tracing::warn!(?e, "事件投递失败（不影响响应）");
    }
}

/// 计费事件投递（announce / xreport 主链路专用）。
///
/// 先补历史欠账，本条失败重试一次，仍失败进进程内重投队列 —— 而不是只
/// `warn!` 后照常回成功：那样这条 announce 的流量增量根本没进流，worker 侧
/// 的 `announce_seen` 幂等键挡得住重投、挡不住「根本没投过来」。
pub(crate) async fn xadd_durable(
    redis: &redis::aio::ConnectionManager,
    stream: &str,
    payload: &serde_json::Value,
) {
    drain_spill(redis).await;
    if xadd_once(redis, stream, payload).await.is_ok() {
        return;
    }
    if xadd_once(redis, stream, payload).await.is_ok() {
        return;
    }
    let Ok(mut q) = spill().lock() else {
        tracing::error!(stream, "重投队列锁损坏，本条计费事件丢弃");
        SPILL_DROPPED.fetch_add(1, Ordering::Relaxed);
        return;
    };
    if q.len() >= SPILL_MAX {
        // 有界：极端长时间 Redis 不可用时丢最旧的并计数上报，
        // 不能让它把进程内存吃穿（那是把一次故障升级成整站故障）。
        q.pop_front();
        SPILL_DROPPED.fetch_add(1, Ordering::Relaxed);
    }
    q.push_back((stream.to_string(), payload.clone()));
    SPILL_DEPTH.store(q.len() as u64, Ordering::Relaxed);
    tracing::warn!(stream, depth = q.len(), "计费事件进重投队列（Redis 未收）");
}

/// 排空重投队列（单轮上限，Redis 刚恢复时不一次灌满）。
/// 锁只在取出/放回时持有，不在 `.await` 上持有。
async fn drain_spill(redis: &redis::aio::ConnectionManager) {
    let batch: Vec<(String, serde_json::Value)> = match spill().lock() {
        Ok(q) if q.is_empty() => return,
        Ok(mut q) => {
            let n = q.len().min(SPILL_DRAIN_BATCH);
            q.drain(..n).collect()
        }
        Err(_) => return,
    };
    let mut back: Vec<(String, serde_json::Value)> = Vec::new();
    let mut sent = 0usize;
    for item in batch {
        if xadd_once(redis, &item.0, &item.1).await.is_ok() {
            sent += 1;
        } else {
            back.push(item);
        }
    }
    if sent > 0 {
        tracing::info!(sent, "重投队列补投成功");
    }
    if !back.is_empty() {
        if let Ok(mut q) = spill().lock() {
            // 放回队首并保持原顺序（欠账必须先于新事件入账）
            for item in back.into_iter().rev() {
                q.push_front(item);
            }
            SPILL_DEPTH.store(q.len() as u64, Ordering::Relaxed);
        }
    }
}

fn xadd_cmd<'a>(stream: &'a str, payload: &'a serde_json::Value) -> redis::Cmd {
    let mut cmd = redis::cmd("XADD");
    // 裁剪只发生在 worker 的消费轮里（announce_main 每轮按最旧未确认条目
    // 裁，见 group::trim_safe），worker 停机/落后时这条流与 session、首页
    // 缓存同库 Redis ⇒ 无界增长会把整站缓存一起拖垮。XADD 侧自己封顶
    // （近似裁剪，开销可忽略）——这是内存保险丝，不是正常裁剪路径。
    cmd.arg(stream)
        .arg("MAXLEN")
        .arg("~")
        .arg(200_000)
        .arg("*")
        .arg("payload")
        .arg(payload.to_string());
    cmd
}

async fn xadd_once(
    redis: &redis::aio::ConnectionManager,
    stream: &str,
    payload: &serde_json::Value,
) -> Result<(), redis::RedisError> {
    let mut conn = redis.clone();
    xadd_cmd(stream, payload).query_async::<()>(&mut conn).await
}

pub(crate) fn bencode_err(msg: &str) -> HttpResponse {
    // 中文按 UTF-8 字节数编码长度（bencode 长度前缀是字节数）
    HttpResponse::Ok().content_type("text/plain").body(format!(
        "d14:failure reason{}:{}e",
        msg.len(),
        msg
    ))
}
