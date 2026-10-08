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
    /// passkey 查询**报错**（≠ 查无此钥）的次数：瞬时抖动与真无效必须分开
    pub(crate) passkey_query_failed: AtomicU64,
    /// `left > 种子大小` 被拒次数（假 announce 的可观测面）
    pub(crate) announce_fake_left: AtomicU64,
    /// peer_id 槽位归属冲突被拒次数（顶号企图）
    pub(crate) announce_peer_taken: AtomicU64,
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

// IP 取信与校验整体搬到 ip_trust.rs（300 行门禁 + 便于单测）；
// 这里再导出，使 announce/scrape 的 `use super::helpers::client_ip` 不变。
pub(crate) use super::ip_trust::client_ip;

/// 内存防护缓存：高频路径不再逐请求打 PG。
/// - passkey：60s TTL（挂起/禁下载最迟 60s 生效，可接受的折衷）
/// - ip_bans / agent_rules / announce_interval：60s 定期刷新
pub(crate) struct GuardInner {
    /// (user_id, download_enabled, suspended, class_id, 缓存时刻)
    pub(crate) passkeys:
        HashMap<String, (i64, bool, bool, i32, Instant)>,
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
