//! FluxTorrent Tracker —— 轻量私有 Tracker（方案 §2.5 自研兜底路线）。
//! 设计（§5.4 announce 链路）：
//!   ① passkey 鉴权：内存缓存（60s TTL）兜底，命中免查 PG，拒绝无效 passkey
//!   ② peer 状态：DashMap 内存表（90s 超时淘汰）
//!   ③ 立即返回 compact 响应（二进制安全），不阻塞计费
//!   ④ 事件 → Redis Stream（XADD），worker 异步消费计费
//! 高频/异常请求防护（announce 路径从 cheap 到 expensive）：
//!   ⓪ ip_bans 封禁：内存缓存（60s 刷新），命中即拒
//!   ⓪' 频率限流：Redis INCR 滑动窗口 —— 每 IP 阈值在 passkey 之前拦截垃圾流量，
//!      每用户阈值在 passkey 之后（Redis 故障 fail-open，不阻断正常 announce）
//!   ①'' agent_rules 黑白名单：内存缓存（60s 刷新），不再逐请求全表扫描
//!   interval / min interval 按 site_settings.announce_interval 下发（BEP3 强制），
//!   告知客户端汇报间隔，从源头抑制客户端高频重发。
//! BEP3 兼容要点：info_hash/peer_id 是任意字节的 percent-encoding ——
//! 绕过 serde Query 反序列化，直接解析原始 query 字节。

mod peers;
mod udp;

use actix_web::{get, web, App, HttpResponse, HttpServer};
use dashmap::DashMap;
use peers::{bencode_scrape, PeerKey, PeerTable};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{RwLock, RwLockReadGuard};
use std::time::{Duration, Instant};

use crate::peers::{bencode_announce, hex, percent_decode, Peer};

pub struct TrackerState {
    pub peers: PeerTable,
    pub redis: redis::aio::ConnectionManager,
    db: sqlx::PgPool,
    guard: RwLock<GuardInner>,
    cfg: GuardCfg,
    metrics: Metrics,
    /// Redis 故障时的本地降级限流窗口（单进程语义，tracker 单实例）
    local: LocalWindows,
    /// 管理端变更通知（flux:guard:ver 版本轮询）→ 立即刷新防护缓存
    force_refresh: AtomicBool,
    ver: AtomicI64,
}

/// Prometheus 指标（/metrics，需 ANN_METRICS_TOKEN）
#[derive(Default)]
struct Metrics {
    announce_total: AtomicU64,
    announce_ip_banned: AtomicU64,
    announce_limited_ip: AtomicU64,
    announce_limited_user: AtomicU64,
    announce_auth_fail: AtomicU64,
    announce_agent_blocked: AtomicU64,
    announce_global_shed: AtomicU64,
    scrape_total: AtomicU64,
    /// Redis 故障 → 本地降级限流的触发次数（fail-open 窗口监测）
    redis_fallback: AtomicU64,
}

/// 本地滑动窗口限流（Redis 故障降级用）：key → (计数, 窗口起点)
struct LocalWindows {
    inner: DashMap<String, (u64, Instant)>,
}

impl LocalWindows {
    /// true = 超限
    fn over(&self, key: &str, limit: i64) -> bool {
        if self.inner.len() > 100_000 {
            self.inner
                .retain(|_, v| v.1.elapsed() < Duration::from_secs(60));
        }
        let mut e = self
            .inner
            .entry(key.to_string())
            .or_insert((0u64, Instant::now()));
        if e.1.elapsed() >= Duration::from_secs(60) {
            *e.value_mut() = (1, Instant::now());
            return false;
        }
        e.0 += 1;
        e.0 as i64 > limit
    }
}

/// 限流/间隔阈值（env 可调，均有兜底默认值）
struct GuardCfg {
    /// 每用户每分钟 announce 上限
    user_per_min: i64,
    /// 每 IP 每分钟 announce 上限（passkey 校验之前拦截，防垃圾流量打 DB/缓存）
    ip_per_min: i64,
    /// 全局应急熔断：全体 announce 每分钟总上限；0 = 关闭（防分布式洪水打垮后端时临时开启）
    global_per_min: i64,
    /// site_settings 缺失 announce_interval 时的兜底值（秒）
    default_interval: i64,
}

fn env_i64(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// 内存防护缓存：高频路径不再逐请求打 PG。
/// - passkey：60s TTL（挂起/禁下载最迟 60s 生效，可接受的折衷）
/// - ip_bans / agent_rules / announce_interval：60s 定期刷新
struct GuardInner {
    passkeys: HashMap<String, (i64, bool, bool, Instant)>,
    ip_bans: HashMap<String, String>,
    /// None = 尚未完成首次加载（0071：正则编译后的 agent/peer_id 交叉规则）
    agent_rules: Option<Vec<AgentRule>>,
    announce_interval: i64,
    refreshed_at: Instant,
}

/// P0-7 客户端规则（NP `agent_allowed_family` 口径精简版）：
/// User-Agent 与 peer_id 各一条正则；吸血客户端常 UA 报正常客户端而 peer_id 暴露 -XL/-XF 前缀，
/// 单查 UA 会漏。规则正则无效时加载阶段即跳过（refresh 时 warn）。
#[derive(Clone)]
struct AgentRule {
    deny: bool,
    agent_re: Option<regex::Regex>,
    peer_re: Option<regex::Regex>,
}

const GUARD_REFRESH: Duration = Duration::from_secs(60);
const PASSKEY_TTL: Duration = Duration::from_secs(60);
const PASSKEY_CACHE_CAP: usize = 50_000;

impl TrackerState {
    fn guard_read(&self) -> RwLockReadGuard<'_, GuardInner> {
        self.guard.read().unwrap_or_else(|e| e.into_inner())
    }

    fn guard_write(&self) -> std::sync::RwLockWriteGuard<'_, GuardInner> {
        self.guard.write().unwrap_or_else(|e| e.into_inner())
    }

    /// ip_bans / agent_rules / announce_interval 刷新（60s 节流；多数请求直接命中缓存返回）。
    /// 管理端变更（flux:guard:ver 轮询置位 force_refresh）可立即触发。
    /// 单项查询失败保留旧值，不做破坏性覆盖。
    async fn refresh_guard(&self) {
        let stale = {
            let g = self.guard_read();
            !self.force_refresh.load(Ordering::Relaxed)
                && g.refreshed_at.elapsed() < GUARD_REFRESH
                && g.agent_rules.is_some()
        };
        if stale {
            return;
        }
        self.force_refresh.store(false, Ordering::Relaxed);
        let bans: Option<Vec<(String, String)>> =
            sqlx::query_as("SELECT host(ip), COALESCE(reason, '') FROM ip_bans")
                .fetch_all(&self.db)
                .await
                .ok();
        let rules: Option<Vec<AgentRule>> = sqlx::query_as::<_, (String, String, String)>(
            "SELECT mode, pattern, COALESCE(peer_id_pattern, '') FROM agent_rules",
        )
        .fetch_all(&self.db)
        .await
        .ok()
        .map(|rows| {
            rows.into_iter()
                .filter_map(|(mode, agent, peer)| {
                    let agent_re = if agent.is_empty() {
                        None
                    } else {
                        match regex::Regex::new(&agent) {
                            Ok(r) => Some(r),
                            Err(e) => {
                                tracing::warn!(%agent, %e, "agent_rules 正则无效，规则跳过");
                                return None;
                            }
                        }
                    };
                    let peer_re = if peer.is_empty() {
                        None
                    } else {
                        match regex::Regex::new(&peer) {
                            Ok(r) => Some(r),
                            Err(e) => {
                                tracing::warn!(%peer, %e, "agent_rules peer_id 正则无效，规则跳过");
                                return None;
                            }
                        }
                    };
                    Some(AgentRule {
                        deny: mode == "deny",
                        agent_re,
                        peer_re,
                    })
                })
                .collect()
        });
        let interval: i64 = sqlx::query_scalar::<_, String>(
            "SELECT value FROM site_settings WHERE name = 'announce_interval'",
        )
        .fetch_optional(&self.db)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<i64>().ok())
        .map(|v| v.clamp(60, 86400))
        .unwrap_or(self.cfg.default_interval);

        let mut g = self.guard_write();
        if let Some(b) = bans {
            g.ip_bans = b.into_iter().collect();
        }
        if let Some(r) = rules {
            g.agent_rules = Some(r);
        }
        g.announce_interval = interval;
        g.refreshed_at = Instant::now();
    }

    /// passkey → (user_id, download_enabled, suspended)，60s 内存缓存
    pub async fn resolve_passkey_cached(&self, passkey: &str) -> Option<(i64, bool, bool)> {
        {
            let g = self.guard_read();
            if let Some((uid, de, su, at)) = g.passkeys.get(passkey) {
                if at.elapsed() < PASSKEY_TTL {
                    return Some((*uid, *de, *su));
                }
            }
        }
        let row: Option<(i64, bool, bool)> = sqlx::query_as::<_, (i64, bool, bool)>(
            "SELECT id, download_enabled, suspended FROM users WHERE passkey = $1 AND status < 2",
        )
        .bind(passkey)
        .fetch_one(&self.db)
        .await
        .ok();
        if let Some(v) = &row {
            let mut g = self.guard_write();
            if g.passkeys.len() >= PASSKEY_CACHE_CAP {
                g.passkeys.clear(); // 粗暴防膨胀：正常站点远达不到该量级
            }
            g.passkeys
                .insert(passkey.to_string(), (v.0, v.1, v.2, Instant::now()));
        }
        row
    }

    /// ip_bans 命中 → 封禁理由
    pub fn ip_banned(&self, ip: &str) -> Option<String> {
        self.guard_read().ip_bans.get(ip).cloned()
    }

    /// agent_rules 黑白名单判定（P0-7 交叉验证版）：
    /// deny 命中（UA 或 peer_id 任一命中 deny 规则）即拒；
    /// allow 列表非空时须命中 allow 的 UA **或** peer_id 条件，未命中 allow 也拒。
    /// 仅当规则从未成功加载（启动后 DB 一直不可达）才放行；一旦有快照，
    /// 刷新失败时 refresh_guard 保留旧值，DB 抖动期间名单持续生效（不再 fail-open）。
    /// peer_id 传可读形式（lossy）以匹配 -XL0014- 等前缀。
    pub fn agent_blocked(&self, agent: Option<&str>, peer_id: &str) -> Option<String> {
        let rules = self.guard_read().agent_rules.clone()?;
        let a = agent.unwrap_or("");
        let hit = |r: &&AgentRule| {
            let agent_ok = r.agent_re.as_ref().is_some_and(|re| re.is_match(a));
            let peer_ok = r.peer_re.as_ref().is_some_and(|re| re.is_match(peer_id));
            agent_ok || peer_ok
        };
        let (allows, denies): (Vec<_>, Vec<_>) = rules.iter().partition(|r| !r.deny);
        if denies.iter().any(hit) {
            return Some("客户端被禁止（黑名单），请联系管理组".into());
        }
        if !allows.is_empty() && !allows.iter().any(hit) {
            return Some("客户端不在允许列表，请联系管理组".into());
        }
        None
    }

    /// 统一限流窗口：Redis INCR 优先；Redis 故障时降级到本地窗口（不再 fail-open 裸放）。
    /// 返回 true = 超限。
    async fn rate_over(&self, key: &str, limit: i64) -> bool {
        use redis::AsyncCommands;
        let mut c = self.redis.clone();
        match c.incr::<_, _, i64>(key, 1).await {
            Ok(n) => {
                if n == 1 {
                    let _ = c.expire::<_, i64>(key, 60).await;
                }
                n > limit
            }
            Err(_) => {
                self.metrics.redis_fallback.fetch_add(1, Ordering::Relaxed);
                self.local.over(key, limit)
            }
        }
    }

    /// 每 IP 频率限流。超限返回失败文案。
    pub async fn rate_limited_ip(&self, ip: &str) -> Option<&'static str> {
        let k = format!("rl:ann:ip:{ip}");
        self.rate_over(&k, self.cfg.ip_per_min)
            .await
            .then_some("announce 频率超限（IP），请稍后再试")
    }

    /// 每用户频率限流（按 user_id 而非 IP —— NAT 场景按 IP 会误伤）。
    pub async fn rate_limited_user(&self, user_id: i64) -> Option<&'static str> {
        let k = format!("rl:ann:u:{user_id}");
        self.rate_over(&k, self.cfg.user_per_min)
            .await
            .then_some("announce 频率超限（用户），请稍后再试")
    }

    /// 全局应急熔断（ANN_RATE_GLOBAL_PER_MIN，0=关闭）：分布式洪水时保护后端不被打垮
    pub async fn global_shed(&self) -> bool {
        if self.cfg.global_per_min <= 0 {
            return false;
        }
        self.rate_over("rl:ann:global", self.cfg.global_per_min)
            .await
    }

    /// (interval, min_interval)：min 取 interval 一半，夹在 [30, 3600]
    pub fn intervals(&self) -> (i64, i64) {
        let v = self.guard_read().announce_interval;
        (v, (v / 2).clamp(30, 3600))
    }
}

/// 从原始 query string 提取参数（字节层，percent-decode）
struct RawParams<'a> {
    raw: &'a str,
}

impl<'a> RawParams<'a> {
    fn new(raw: &'a str) -> Self {
        Self { raw }
    }
    /// 返回解码后的字节值
    fn get_bytes(&self, key: &str) -> Option<Vec<u8>> {
        for pair in self.raw.split('&') {
            let mut it = pair.splitn(2, '=');
            if it.next() == Some(key) {
                let v = it.next().unwrap_or("");
                return Some(percent_decode(v.as_bytes()));
            }
        }
        None
    }
    fn get_str(&self, key: &str) -> Option<String> {
        self.get_bytes(key)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    }
    fn get_i64(&self, key: &str, default: i64) -> i64 {
        self.get_str(key)
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }
}

#[get("/announce/{passkey}")]
async fn announce(
    state: web::Data<TrackerState>,
    path: web::Path<String>,
    req: actix_web::HttpRequest,
) -> HttpResponse {
    let passkey = path.into_inner();
    state.metrics.announce_total.fetch_add(1, Ordering::Relaxed);
    let raw_query = req.query_string();

    let params = RawParams::new(raw_query);
    let Some(info_hash_raw) = params.get_bytes("info_hash") else {
        return bencode_err("缺少 info_hash");
    };
    if info_hash_raw.len() != 20 {
        return bencode_err("info_hash 必须为 20 字节");
    }
    let Some(peer_id_raw) = params.get_bytes("peer_id") else {
        return bencode_err("缺少 peer_id");
    };
    if peer_id_raw.is_empty() || peer_id_raw.len() > 20 {
        return bencode_err("peer_id 长度无效");
    }

    let info_hash_hex = hex(&info_hash_raw);
    let peer_id_hex = hex(&peer_id_raw);
    let port: u16 = params.get_i64("port", 0) as u16;
    let uploaded = params.get_i64("uploaded", 0);
    let downloaded = params.get_i64("downloaded", 0);
    let left = params.get_i64("left", 0);
    let numwant = params.get_i64("numwant", 50).clamp(1, 200) as usize;
    let event = params.get_str("event").unwrap_or_default();
    let event = event.as_str();
    // 安全（P2）：不信任客户端自报 IP —— 仅显式配置代理时才采用参数值
    let trust_param_ip = std::env::var("TRUST_PROXY_IP").unwrap_or_default() == "1";
    let ip = if trust_param_ip {
        params
            .get_str("ip")
            .or_else(|| req.peer_addr().map(|a| a.ip().to_string()))
    } else {
        req.peer_addr().map(|a| a.ip().to_string())
    }
    .unwrap_or_else(|| "0.0.0.0".into());

    // ⓪ 应急熔断（默认关闭，见 ANN_RATE_GLOBAL_PER_MIN）
    if state.global_shed().await {
        state
            .metrics
            .announce_global_shed
            .fetch_add(1, Ordering::Relaxed);
        return bencode_err("tracker 负载保护已触发，请稍后再试");
    }

    // ⓪ 防护缓存热身（内部 60s 节流，命中缓存时近乎零开销）
    state.refresh_guard().await;

    // ⓪ IP 封禁（内存缓存，高频路径零 DB 开销）
    if let Some(reason) = state.ip_banned(&ip) {
        state
            .metrics
            .announce_ip_banned
            .fetch_add(1, Ordering::Relaxed);
        return bencode_err(&format!("IP 已被封禁：{reason}"));
    }
    // ⓪' 每 IP 频率（在 passkey 之前拦垃圾流量，防无效 passkey 洪水打缓存/DB）
    if let Some(msg) = state.rate_limited_ip(&ip).await {
        state
            .metrics
            .announce_limited_ip
            .fetch_add(1, Ordering::Relaxed);
        return bencode_err(msg);
    }

    // ① passkey → user_id + 管理开关（内存缓存 60s，命中免查 PG）
    let Some((user_id, download_enabled, suspended)) = state.resolve_passkey_cached(&passkey).await
    else {
        state
            .metrics
            .announce_auth_fail
            .fetch_add(1, Ordering::Relaxed);
        return bencode_err("passkey 无效，请在站点重置");
    };
    if suspended {
        return bencode_err("账号已挂起，请联系管理组");
    }
    if !download_enabled && left > 0 {
        return bencode_err("下载权限已被禁用，请联系管理组");
    }

    // ①' 每用户频率（按 user_id 而非 IP —— NAT 场景按 IP 会误伤）
    if let Some(msg) = state.rate_limited_user(user_id).await {
        state
            .metrics
            .announce_limited_user
            .fetch_add(1, Ordering::Relaxed);
        return bencode_err(msg);
    }

    // ①'' 客户端黑名单（G-06 / P0-7）：UA 与 peer_id 双正则交叉匹配；
    // 执行闭环（0069）：命中事件经 Redis 去重（每 user+agent 1h 一条）后投递 worker 落 cheat_events，
    // 管理组在后台可查 —— 不再是"拒绝即止、无处留痕"。
    let peer_id_readable = String::from_utf8_lossy(&peer_id_raw).into_owned();
    // 审计修复（P1）：BT 客户端把 UA 放 HTTP 头而非 query 参数 —— 旧版只读 ?agent=，
    // agent_rules 的 UA 正则恒不命中、snatches.agent 恒空。改为 UA 头优先、query 兜底。
    let agent_str = req
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .or_else(|| params.get_str("agent"))
        .unwrap_or_default();
    if let Some(reason) = state.agent_blocked(Some(agent_str.as_str()), &peer_id_readable) {
        state
            .metrics
            .announce_agent_blocked
            .fetch_add(1, Ordering::Relaxed);
        emit_agent_block(&state.redis, user_id, &agent_str, &ip, &reason).await;
        return bencode_err(&reason);
    }

    let key = PeerKey {
        info_hash: info_hash_hex.clone(),
        peer_id: peer_id_hex.clone(),
    };

    // stopped：移除 peer；其余 upsert
    if event == "stopped" {
        state.peers.remove(&key);
    } else {
        state.peers.upsert(Peer {
            key: key.clone(),
            ip: ip.clone(),
            port,
            uploaded,
            downloaded,
            left,
            last_seen: chrono::Utc::now(),
            user_id,
            connectable: peers::CONN_UNTESTED, // upsert 内部会保留既有测量值
        });
    }

    // ④ 事件投递（fire-and-forget，失败仅告警；ip/conn 供 worker 反作弊分析）
    emit_event(
        &state.redis,
        &info_hash_hex,
        user_id,
        uploaded,
        downloaded,
        event,
        left,
        &ip,
        state.peers.connectable_of(&key),
        &agent_str,
    )
    .await;

    // ③ compact 二进制响应（interval 按 site_settings.announce_interval 下发；BEP-7 v6 进 peers6）
    let (interval, min_interval) = state.intervals();
    let body = if event == "stopped" {
        bencode_announce(0, 0, 0, &[], &[], interval, min_interval)
    } else {
        let seeders = state.peers.count_seeders(&info_hash_hex);
        let leechers = state.peers.count_leechers(&info_hash_hex);
        let snap = state.peers.snapshot(&info_hash_hex, numwant, &key.peer_id);
        bencode_announce(
            seeders as i64,
            leechers as i64,
            0,
            &snap.v4,
            &snap.v6,
            interval,
            min_interval,
        )
    };
    HttpResponse::Ok().content_type("text/plain").body(body)
}

#[get("/scrape/{passkey}")]
async fn scrape(
    state: web::Data<TrackerState>,
    path: web::Path<String>,
    req: actix_web::HttpRequest,
) -> HttpResponse {
    let passkey = path.into_inner();
    state.metrics.scrape_total.fetch_add(1, Ordering::Relaxed);
    let ip = req
        .peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "0.0.0.0".into());

    state.refresh_guard().await;
    if let Some(reason) = state.ip_banned(&ip) {
        return bencode_err(&format!("IP 已被封禁：{reason}"));
    }
    if let Some(msg) = state.rate_limited_ip(&ip).await {
        return bencode_err(msg);
    }
    if state.resolve_passkey_cached(&passkey).await.is_none() {
        return bencode_err("passkey 无效");
    }
    let params = RawParams::new(req.query_string());
    // BEP48：info_hash 可重复出现多次，每次为 20 字节 percent-encoding
    let mut files: Vec<(Vec<u8>, usize, usize)> = Vec::new();
    for pair in req.query_string().split('&') {
        let mut it = pair.splitn(2, '=');
        if it.next() == Some("info_hash") {
            let raw = percent_decode(it.next().unwrap_or("").as_bytes());
            if raw.len() == 20 {
                let hexkey = hex(&raw);
                let (s, l) = state.peers.counts(&hexkey);
                files.push((raw, s, l));
            }
        }
    }
    let _ = params;
    HttpResponse::Ok()
        .content_type("text/plain")
        .body(bencode_scrape(&files))
}

/// Prometheus 指标端点：ANN_METRICS_TOKEN 未设置时返回 404（不暴露）；
/// 抓取端需带 X-Metrics-Token 头。监控与告警基线见 生产部署指南「监控与告警」节。
#[get("/metrics")]
async fn metrics(state: web::Data<TrackerState>, req: actix_web::HttpRequest) -> HttpResponse {
    let tok = std::env::var("ANN_METRICS_TOKEN").unwrap_or_default();
    if tok.is_empty()
        || req
            .headers()
            .get("x-metrics-token")
            .and_then(|v| v.to_str().ok())
            != Some(tok.as_str())
    {
        return HttpResponse::NotFound().finish();
    }
    let m = &state.metrics;
    // 全量 GC 挂在抓取周期（P0-3：平时各桶惰性清理，这里兜底回收空桶与超时 peer）
    state.peers.gc_all();
    let (passkey_cache, ip_bans, agent_rules, swarms) = {
        let g = state.guard_read();
        (
            g.passkeys.len(),
            g.ip_bans.len(),
            g.agent_rules.as_ref().map_or(0, |r| r.len()),
            state.peers.swarms(),
        )
    };
    let body = format!(
        concat!(
            "# TYPE flux_tracker_announce_total counter\n",
            "flux_tracker_announce_total {}\n",
            "# TYPE flux_tracker_announce_ip_banned_total counter\n",
            "flux_tracker_announce_ip_banned_total {}\n",
            "# TYPE flux_tracker_announce_limited_ip_total counter\n",
            "flux_tracker_announce_limited_ip_total {}\n",
            "# TYPE flux_tracker_announce_limited_user_total counter\n",
            "flux_tracker_announce_limited_user_total {}\n",
            "# TYPE flux_tracker_announce_auth_fail_total counter\n",
            "flux_tracker_announce_auth_fail_total {}\n",
            "# TYPE flux_tracker_announce_agent_blocked_total counter\n",
            "flux_tracker_announce_agent_blocked_total {}\n",
            "# TYPE flux_tracker_announce_global_shed_total counter\n",
            "flux_tracker_announce_global_shed_total {}\n",
            "# TYPE flux_tracker_scrape_total counter\n",
            "flux_tracker_scrape_total {}\n",
            "# TYPE flux_tracker_redis_fallback_total counter\n",
            "flux_tracker_redis_fallback_total {}\n",
            "# TYPE flux_tracker_peers_active gauge\n",
            "flux_tracker_peers_active {}\n",
            "# TYPE flux_tracker_swarms_active gauge\n",
            "flux_tracker_swarms_active {}\n",
            "# TYPE flux_tracker_guard_cache gauge\n",
            "flux_tracker_guard_cache{{kind=\"passkey\"}} {}\n",
            "flux_tracker_guard_cache{{kind=\"ip_ban\"}} {}\n",
            "flux_tracker_guard_cache{{kind=\"agent_rule\"}} {}\n",
        ),
        m.announce_total.load(Ordering::Relaxed),
        m.announce_ip_banned.load(Ordering::Relaxed),
        m.announce_limited_ip.load(Ordering::Relaxed),
        m.announce_limited_user.load(Ordering::Relaxed),
        m.announce_auth_fail.load(Ordering::Relaxed),
        m.announce_agent_blocked.load(Ordering::Relaxed),
        m.announce_global_shed.load(Ordering::Relaxed),
        m.scrape_total.load(Ordering::Relaxed),
        m.redis_fallback.load(Ordering::Relaxed),
        state.peers.len(),
        swarms,
        passkey_cache,
        ip_bans,
        agent_rules,
    );
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(body)
}

fn bencode_err(msg: &str) -> HttpResponse {
    // 中文按 UTF-8 字节数编码长度（bencode 长度前缀是字节数）
    HttpResponse::Ok().content_type("text/plain").body(format!(
        "d14:failure reason{}:{}e",
        msg.len(),
        msg
    ))
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
) {
    let mut payload = serde_json::json!({
        "user": user, "hash": info_hash_hex, "up": up, "down": down,
        "event": event, "left": left,
        "ts": chrono::Utc::now().to_rfc3339(),
    });
    // 0071：仅在有测量值时携带（worker 端 Option 语义：缺省 = 不覆盖 snatches.connectable）
    if !ip.is_empty() {
        payload["ip"] = serde_json::json!(ip);
    }
    if conn != peers::CONN_UNTESTED {
        payload["conn"] = serde_json::json!(conn);
    }
    // 0098：BT 客户端 UA（下载列表「客户端」列 + 反作弊证据）；截断防事件膨胀
    if !agent.is_empty() {
        payload["agent"] = serde_json::json!(&agent[..agent.len().min(200)]);
    }
    xadd(redis, "flux:announce", &payload).await;
}

/// agent_rules 命中投递：同 (user, agent) 1 小时去重（SET NX EX），命中才 XADD flux:agent_block。
/// worker 消费落 cheat_events（hits 累加 / 首次进管理组信箱）—— 高频拒绝路径零 DB 开销。
async fn emit_agent_block(
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

async fn xadd(redis: &redis::aio::ConnectionManager, stream: &str, payload: &serde_json::Value) {
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

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let bind = std::env::var("TRACKER_BIND").unwrap_or_else(|_| "0.0.0.0:7070".into());
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://flux:fluxdevpass@127.0.0.1:5432/fluxtorrent".into());
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());

    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&db_url)
        .await?;
    let redis = redis::Client::open(redis_url.as_str())?
        .get_connection_manager()
        .await?;
    let cfg = GuardCfg {
        // 保种大户口径：interval 1800s 时，每分钟 announce 数 ≈ 挂种数/30。
        // 1800/min 覆盖 5.4 万挂种；超限只是计费延迟（累计量差值口径，不丢），且账号可挂起兜底。
        user_per_min: env_i64("ANN_RATE_USER_PER_MIN", 1800),
        // 每 IP 兜底（兼容量子洞 NAT / 多做种盒 / 单 IP 一鲸鱼+常量用户）；
        // 无凭据垃圾洪水仍被限在 60r/s/IP，分布式洪水属边缘网关/WAF 职责。
        ip_per_min: env_i64("ANN_RATE_IP_PER_MIN", 3600),
        // 全局应急熔断：0=关闭；遭分布式洪水时临时设置（如 120000 = 2000r/s 总闸）
        global_per_min: env_i64("ANN_RATE_GLOBAL_PER_MIN", 0),
        default_interval: env_i64("ANN_INTERVAL_DEFAULT", 1800).clamp(60, 86400),
    };
    let state = web::Data::new(TrackerState {
        peers: PeerTable::new(),
        redis: redis.clone(),
        db,
        guard: RwLock::new(GuardInner {
            passkeys: HashMap::new(),
            ip_bans: HashMap::new(),
            agent_rules: None, // None 使首个请求必然触发首次加载（见 refresh_guard 的 stale 判定）
            announce_interval: cfg.default_interval,
            refreshed_at: Instant::now(),
        }),
        cfg,
        metrics: Metrics::default(),
        local: LocalWindows {
            inner: DashMap::new(),
        },
        force_refresh: AtomicBool::new(false),
        ver: AtomicI64::new(0),
    });

    // peer 快照预热（0101）：重启后从 Redis 恢复未超时 peer，缩短做种列表空窗。
    // 客户端 30min 内重 announce 本就可自愈——恢复失败仅记日志，绝不阻塞启动。
    {
        use redis::AsyncCommands;
        let mut c = redis.clone();
        match c.get::<_, Option<String>>("flux:tracker:peers").await {
            Ok(Some(raw)) => match serde_json::from_str::<Vec<(String, Vec<Peer>)>>(&raw) {
                Ok(snap) => {
                    let n = state.peers.restore(snap);
                    tracing::info!(n, "peer table warm-restored from redis");
                }
                Err(e) => tracing::warn!(%e, "peer snapshot parse failed, skip warm restore"),
            },
            Ok(None) => {}
            Err(e) => tracing::warn!(%e, "peer snapshot read failed, skip warm restore"),
        }
    }

    // peer 快照周期落盘（60s）：tracker 是 SPOF，快照让重启从「全量重建」变「增量补齐」
    {
        let st = state.clone();
        actix_web::rt::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(60));
            loop {
                tick.tick().await;
                let snap = st.peers.export();
                match serde_json::to_string(&snap) {
                    Ok(raw) => {
                        let mut c = st.redis.clone();
                        use redis::AsyncCommands;
                        // 30min TTL：tracker 长时间下线后旧快照不再有效
                        if let Err(e) = c.set_ex::<_, _, ()>("flux:tracker:peers", raw, 1800).await
                        {
                            tracing::warn!(%e, "peer snapshot write failed");
                        }
                    }
                    Err(e) => tracing::warn!(%e, "peer snapshot encode failed"),
                }
            }
        });
    }

    // 管理端变更通知轮询：flux:guard:ver 每 3s 一查（单 GET，可忽略的开销）。
    // API 侧在 ip_bans/agent_rules/挂起/passkey 变更后 INCR 该键 → 立即刷新缓存+清 passkey。
    {
        let st = state.clone();
        actix_web::rt::spawn(async move {
            use redis::AsyncCommands;
            let mut conn = st.redis.clone();
            let mut tick = tokio::time::interval(Duration::from_secs(3));
            loop {
                tick.tick().await;
                if let Ok(Some(v)) = conn.get::<_, Option<i64>>("flux:guard:ver").await {
                    let last = st.ver.swap(v, Ordering::Relaxed);
                    if last != v {
                        st.force_refresh.store(true, Ordering::Relaxed);
                        st.guard_write().passkeys.clear();
                    }
                }
            }
        });
    }

    // connectable 抽样（0071 P1-9，防假保种）：每 5min 抽 50 个 peer 做 TCP 回连（3s 超时），
    // 未测优先、已测轮替复测。结果写回 peer 表 → 随 announce 事件流入 snatches.connectable，
    // 「不可达 + 零上传」的做种不计做种收益并进作弊探测。纯探测不阻断任何响应路径。
    {
        let st = state.clone();
        actix_web::rt::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(300));
            loop {
                tick.tick().await;
                for (key, probe_ip, probe_port) in st.peers.sample_probes(50) {
                    let attempt = tokio::time::timeout(
                        Duration::from_secs(3),
                        tokio::net::TcpStream::connect((probe_ip.as_str(), probe_port)),
                    )
                    .await;
                    let reachable = matches!(attempt, Ok(Ok(_)));
                    st.peers.set_connectable(&key, reachable);
                }
            }
        });
    }

    // UDP tracker（BEP15）：TRACKER_UDP_BIND 未设置（空）则不启用；
    // 默认 6969。与 HTTP announce 共享 state（peer 表/限流/事件流）。
    let udp_bind = std::env::var("TRACKER_UDP_BIND").unwrap_or_else(|_| "0.0.0.0:6969".into());
    if !udp_bind.is_empty() {
        let udp = std::sync::Arc::new(udp::UdpTracker::new(state.clone()));
        let ub = udp_bind.clone();
        actix_web::rt::spawn(async move {
            if let Err(e) = udp.run(&ub).await {
                tracing::error!(%e, %ub, "UDP tracker exited");
            }
        });
    }

    tracing::info!(
        "flux-tracker listening on {bind}（防护：每用户 {}/min，每 IP {}/min，interval {}s）",
        state.cfg.user_per_min,
        state.cfg.ip_per_min,
        state.cfg.default_interval
    );
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(announce)
            .service(scrape)
            .service(metrics)
    })
    .bind(&bind)?
    .run()
    .await?;
    Ok(())
}
