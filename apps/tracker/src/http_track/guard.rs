//! 防护缓存刷新（passkey / ip_bans / agent_rules / interval）。
//! 从 helpers.rs 按域拆出。

use std::sync::atomic::Ordering;
use std::time::Instant;

use super::helpers::{
    AgentRule, TrackerState, GUARD_REFRESH, PASSKEY_CACHE_CAP, PASSKEY_TTL,
};

impl TrackerState {
    /// ip_bans / agent_rules / announce_interval 刷新（60s 节流；多数请求直接命中缓存返回）。
    /// 管理端变更（flux:guard:ver 轮询置位 force_refresh）可立即触发。
    /// 单项查询失败保留旧值，不做破坏性覆盖。
    pub(crate) async fn refresh_guard(&self) {
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
        let bans: Option<Vec<(String, String)>> = sqlx::query_as(
            "SELECT host(ip), COALESCE(reason, '') FROM ip_bans",
        )
        .fetch_all(&self.db)
        .await
        .ok();
        let rules: Option<Vec<AgentRule>> = sqlx::query_as::<_, (String, String, String)>(
                        "SELECT mode, pattern, \
             COALESCE(peer_id_pattern, '') FROM agent_rules",
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
    pub async fn resolve_passkey_cached(
        &self,
        passkey: &str,
    ) -> Option<(i64, bool, bool)> {
        {
            let g = self.guard_read();
            if let Some((uid, de, su, at)) = g.passkeys.get(passkey) {
                if at.elapsed() < PASSKEY_TTL {
                    return Some((*uid, *de, *su));
                }
            }
        }
        let row: Option<(i64, bool, bool)> =
            sqlx::query_as::<_, (i64, bool, bool)>(
                "SELECT id, download_enabled, \
             suspended FROM users WHERE passkey = $1 AND status < 2",
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
    pub fn agent_blocked(
        &self,
        agent: Option<&str>,
        peer_id: &str,
    ) -> Option<String> {
        let rules = self.guard_read().agent_rules.clone()?;
        let a = agent.unwrap_or("");
        let hit = |r: &&AgentRule| {
            let agent_ok = r.agent_re.as_ref().is_some_and(|re| re.is_match(a));
            let peer_ok =
                r.peer_re.as_ref().is_some_and(|re| re.is_match(peer_id));
            agent_ok || peer_ok
        };
        let (allows, denies): (Vec<_>, Vec<_>) =
            rules.iter().partition(|r| !r.deny);
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
    pub async fn rate_limited_user(
        &self,
        user_id: i64,
    ) -> Option<&'static str> {
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
