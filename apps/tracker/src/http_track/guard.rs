//! passkey 缓存与限流窗口。
//! 刷新与种子白名单在 guard_refresh.rs，段封禁/元数据/负缓存在 guard_store.rs。

use std::sync::atomic::Ordering;
use std::time::Instant;

use super::guard_store;
use super::helpers::{
    AgentRule, TrackerState, PASSKEY_CACHE_CAP, PASSKEY_MISS_TTL, PASSKEY_TTL,
};

impl TrackerState {
    /// passkey → (user_id, download_enabled, suspended, class_id)，60s 内存缓存。
    /// 未命中负缓存 30s（审计 10-06 第 7 条）：随机 passkey 洪水不落 PG。
    pub async fn resolve_passkey_cached(
        &self,
        passkey: &str,
    ) -> Option<(i64, bool, bool, i32)> {
        {
            let g = self.guard_read();
            if let Some((uid, de, su, cls, at)) = g.passkeys.get(passkey) {
                if at.elapsed() < PASSKEY_TTL {
                    return Some((*uid, *de, *su, *cls));
                }
            }
            if g.passkey_miss
                .get(passkey)
                .is_some_and(|at| at.elapsed() < PASSKEY_MISS_TTL)
            {
                return None;
            }
        }
        // 改密后的宽限窗（审计 10-07 P1-4）：passkey 烤在用户已下载的每一个
        // .torrent 里，旧密钥一失效就等于手上所有种子集体停种（libtorrent 系
        // 客户端还会把 tracker 标成错误、长时间不再重试）。prev 由改密接口写入。
        let outcome = sqlx::query_as::<_, (i64, bool, bool, i32)>(
            "SELECT id, download_enabled, suspended, class_id \
             FROM user_by_passkey WHERE passkey = $1 AND status < 2",
        )
        .bind(passkey)
        .fetch_optional(&self.db)
        .await;
        let row = match outcome {
            Ok(r) => r,
            Err(e) => {
                // 查询失败 ≠ 密钥无效。旧实现用 `.ok()` 把两者混成一谈，
                // 于是一次瞬时抖动就把**合法** passkey 负缓存 30s——表现成
                // 全站「passkey 无效，请在站点重置」的假象。
                tracing::warn!(%e, "passkey 查询失败，本次拒绝但不写负缓存");
                self.metrics
                    .passkey_query_failed
                    .fetch_add(1, Ordering::Relaxed);
                return None;
            }
        };
        let mut g = self.guard_write();
        if g.passkeys.len() + g.passkey_miss.len() >= PASSKEY_CACHE_CAP {
            g.passkeys.clear(); // 粗暴防膨胀：正常站点远达不到该量级
            g.passkey_miss.clear();
        }
        match &row {
            Some(v) => {
                g.passkeys
                    .insert(passkey.to_string(), (v.0, v.1, v.2, v.3,
                                                  Instant::now()));
            }
            None => {
                g.passkey_miss.insert(passkey.to_string(), Instant::now());
            }
        }
        row
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
                // INCR 后补 EXPIRE 若只试一次且失败，这个键就永不过期 ⇒
                // 窗口再不重置，该 IP/用户等于被永久限流。pttl<0 即无 TTL，补一次。
                let ttl = c.pttl::<_, i64>(key).await.unwrap_or(-1);
                if ttl < 0 {
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

    /// scrape 独立限流（审计 10-07 P2）：旧版与 announce 共用 `rl:ann:ip:` 桶，
    /// 一次全站轮询就把该 IP 的 announce 额度吃光，用户表现为「突然全站在报超限」。
    pub async fn rate_limited_scrape(
        &self,
        ip: &str,
    ) -> Option<&'static str> {
        let k = format!("rl:scr:ip:{ip}");
        self.rate_over(&k, guard_store::scr_per_min())
            .await
            .then_some("scrape 频率超限（IP），请稍后再试")
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
    /// 同 `(user, torrent)` 的**周期** announce 认领合并窗：
    /// `true` = 窗口内第一条（照常发事件）；`false` = 已在窗内（不发事件，
    /// peer 列表照常回）。`event` 非空（started/completed/stopped）时调用方
    /// 不走这里——事件语义不能并。
    ///
    /// Redis 不可达 ⇒ 一律 `true`（fail-open）：合并只是省 PG 负载，静默丢事件
    /// 会连带丢活跃度与完成数读数，代价比省下的事务大。
    pub async fn claim_event_window(&self, user_id: i64, info_hash: &str) -> bool {
        let window = super::gate::merge_window_secs(
            self.intervals().0,
            super::gate::event_merge_pct(),
        );
        if window <= 0 {
            return true;
        }
        let key = format!("ann:evt:{user_id}:{info_hash}");
        let mut c = self.redis.clone();
        let got: Result<Option<String>, redis::RedisError> = redis::cmd("SET")
            .arg(&key)
            .arg("1")
            .arg("NX")
            .arg("EX")
            .arg(window)
            .query_async(&mut c)
            .await;
        match got {
            // SET NX 有值 = 键原本不存在 ⇒ 这条是窗口内第一条
            Ok(v) => v.is_some(),
            Err(_) => {
                self.metrics.redis_fallback.fetch_add(1, Ordering::Relaxed);
                true
            }
        }
    }

    pub fn intervals(&self) -> (i64, i64) {
        let v = self.guard_read().announce_interval;
        (v, (v / 2).clamp(30, 3600))
    }
}
