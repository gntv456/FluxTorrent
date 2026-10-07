//! bencode 错误 + agent 块投递 + XADD。
//! 从 tracker main.rs 按域拆出。

use crate::peers;
use actix_web::HttpResponse;

/// Prometheus 指标（/metrics，需 ANN_METRICS_TOKEN）

/// 本地滑动窗口限流（Redis 故障降级用）：key → (计数, 窗口起点)

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
    xadd(redis, "flux:announce", &payload).await;
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
    let mut cmd = redis::cmd("XADD");
    cmd.arg(stream)
        .arg("*")
        .arg("payload")
        .arg(payload.to_string());
    let mut conn = redis.clone();
    if let Err(e) = cmd.query_async::<()>(&mut conn).await {
        tracing::warn!(?e, "事件投递失败（不影响响应）");
    }
}

pub(crate) fn bencode_err(msg: &str) -> HttpResponse {
    // 中文按 UTF-8 字节数编码长度（bencode 长度前缀是字节数）
    HttpResponse::Ok().content_type("text/plain").body(format!(
        "d14:failure reason{}:{}e",
        msg.len(),
        msg
    ))
}
