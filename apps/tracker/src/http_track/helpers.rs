//! 环境/限流/封禁缓存助手。
//! 从 tracker main.rs 按域拆出。

use crate::peers::PeerTable;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64};
use std::sync::{RwLock, RwLockReadGuard};
use std::time::{Duration, Instant};

pub struct TrackerState {
    pub peers: PeerTable,
    pub redis: redis::aio::ConnectionManager,
    pub(crate) db: sqlx::PgPool,
    pub(crate) guard: RwLock<GuardInner>,
    pub(crate) cfg: GuardCfg,
    pub(crate) metrics: Metrics,
    /// Redis 故障时的本地降级限流窗口（单进程语义，tracker 单实例）
    pub(crate) local: LocalWindows,
    /// 管理端变更通知（flux:guard:ver 版本轮询）→ 立即刷新防护缓存
    pub(crate) force_refresh: AtomicBool,
    pub(crate) ver: AtomicI64,
}

/// Prometheus 指标（/metrics，需 ANN_METRICS_TOKEN）
#[derive(Default)]
pub(crate) struct Metrics {
    pub(crate) announce_total: AtomicU64,
    pub(crate) announce_ip_banned: AtomicU64,
    pub(crate) announce_limited_ip: AtomicU64,
    pub(crate) announce_limited_user: AtomicU64,
    pub(crate) announce_auth_fail: AtomicU64,
    pub(crate) announce_agent_blocked: AtomicU64,
    pub(crate) announce_torrent_unknown: AtomicU64,
    pub(crate) announce_global_shed: AtomicU64,
    pub(crate) scrape_total: AtomicU64,
    /// Redis 故障 → 本地降级限流的触发次数（fail-open 窗口监测）
    pub(crate) redis_fallback: AtomicU64,
}

/// 本地滑动窗口限流（Redis 故障降级用）：key → (计数, 窗口起点)
pub(crate) struct LocalWindows {
    pub(crate) inner: DashMap<String, (u64, Instant)>,
}

impl LocalWindows {
    /// true = 超限
    pub(crate) fn over(&self, key: &str, limit: i64) -> bool {
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
pub(crate) struct GuardCfg {
    /// 每用户每分钟 announce 上限
    pub(crate) user_per_min: i64,
    /// 每 IP 每分钟 announce 上限（passkey 校验之前拦截，防垃圾流量打 DB/缓存）
    pub(crate) ip_per_min: i64,
    /// 全局应急熔断：全体 announce 每分钟总上限；0 = 关闭（防分布式洪水打垮后端时临时开启）
    pub(crate) global_per_min: i64,
    /// site_settings 缺失 announce_interval 时的兜底值（秒）
    pub(crate) default_interval: i64,
}

pub(crate) fn env_i64(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn trust_xff() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("TRUST_PROXY").unwrap_or_default() == "1")
}

fn trust_param_ip() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        std::env::var("TRUST_PROXY_IP").unwrap_or_default() == "1"
    })
}

/// 客户端 IP 判定（announce/scrape 共用——审计 10-06 第 3 条：scrape 曾只认
/// socket 对端，LB 后全部 peer IP 变 LB 地址，与 announce 口径分叉）。
/// 安全：不信任客户端自报 —— 仅显式配置代理时采用。优先级：
/// TRUST_PROXY=1 时 XFF **右值**（P1-4，2026-10-06 安全审计：旧版取首值，
/// 依赖「反代追加而非覆盖」——nginx 误配透传 `$http_x_forwarded_for` 时
/// 客户端可伪造任意 IP 绕过 ip_bans/限流。右值=直连我们的那台反代追加的
/// 那段，单层反代下即真实客户端 IP）
/// > TRUST_PROXY_IP=1 时 ?ip= 参数 > socket 对端。
pub(crate) fn client_ip(
    req: &actix_web::HttpRequest,
    params: &super::params::RawParams,
) -> String {
    if trust_xff() {
        if let Some(v) = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next_back())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return v.to_string();
        }
    } else if trust_param_ip() {
        // 二轮遗留（2026-10-07）：?ip= 是完全的客户端输入，旧版零校验直接
        // 用作 ip_bans 匹配键/限流键/事件流 ip——误配此开关时攻击者换一个
        // 任意串即打散所有限流桶并注入脏取证。至少做 IP 格式校验：必须是
        // 可解析的 v4/v6 地址（BEP24 语义本来也只允许地址）。
        if let Some(v) = params
            .get_str("ip")
            .filter(|s| !s.is_empty())
            .filter(|s| s.parse::<std::net::IpAddr>().is_ok())
        {
            return v;
        }
    }
    req.peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "0.0.0.0".into())
}

/// 内存防护缓存：高频路径不再逐请求打 PG。
/// - passkey：60s TTL（挂起/禁下载最迟 60s 生效，可接受的折衷）
/// - ip_bans / agent_rules / announce_interval：60s 定期刷新
pub(crate) struct GuardInner {
    pub(crate) passkeys: HashMap<String, (i64, bool, bool, Instant)>,
    /// 无效 passkey 负缓存（审计 10-06 第 7 条）：旧实现只缓存命中——
    /// 随机 passkey 洪水每发必查 PG，缓存形同虚设。
    pub(crate) passkey_miss: HashMap<String, Instant>,
    pub(crate) ip_bans: HashMap<String, String>,
    /// None = 尚未完成首次加载（0071：正则编译后的 agent/peer_id 交叉规则）
    pub(crate) agent_rules: Option<Vec<AgentRule>>,
    pub(crate) announce_interval: i64,
    /// 已注册种子 info_hash 集合（P0-2 白名单）。None = 尚未完成首次加载
    /// （此间 torrent_registered 走直查，见 guard.rs）。
    pub(crate) known_hashes: Option<std::collections::HashSet<String>>,
    pub(crate) refreshed_at: Instant,
}

/// P0-7 客户端规则（NP `agent_allowed_family` 口径精简版）：
/// User-Agent 与 peer_id 各一条正则；吸血客户端常 UA 报正常客户端而 peer_id 暴露 -XL/-XF 前缀，
/// 单查 UA 会漏。规则正则无效时加载阶段即跳过（refresh 时 warn）。
#[derive(Clone)]
pub(crate) struct AgentRule {
    pub(crate) deny: bool,
    pub(crate) agent_re: Option<regex::Regex>,
    pub(crate) peer_re: Option<regex::Regex>,
}

pub(crate) const GUARD_REFRESH: Duration = Duration::from_secs(60);
pub(crate) const PASSKEY_TTL: Duration = Duration::from_secs(60);
pub(crate) const PASSKEY_CACHE_CAP: usize = 50_000;
/// 无效 passkey 负缓存时长（短于正缓存——新注册/重置的 passkey 最迟 30s 生效）
pub(crate) const PASSKEY_MISS_TTL: Duration = Duration::from_secs(30);

impl TrackerState {
    pub(crate) fn guard_read(&self) -> RwLockReadGuard<'_, GuardInner> {
        self.guard.read().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn guard_write(
        &self,
    ) -> std::sync::RwLockWriteGuard<'_, GuardInner> {
        self.guard.write().unwrap_or_else(|e| e.into_inner())
    }
}
