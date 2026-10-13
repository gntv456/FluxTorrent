//! passkey 缓存与限流窗口。
//! 刷新与种子白名单在 guard_refresh.rs，段封禁/元数据/负缓存在 guard_store.rs。

use std::sync::atomic::Ordering;
use std::time::Instant;

use super::agent_ver::{ua_version, version_in_range};
use super::ann_ban::{ban_enabled, ban_key};
use super::guard_store;
use super::helpers::{
    AgentRule, TrackerState, PASSKEY_CACHE_CAP, PASSKEY_MISS_TTL, PASSKEY_TTL,
};

impl TrackerState {
    /// passkey → 用户快照，60s 内存缓存；未命中负缓存 30s（审计 10-06 第 7 条）：
    /// 随机 passkey 洪水不落 PG。快照里除鉴权四件套外还带即时闸门要用的量
    /// （上传/下载/注册天数/观察期/等级门槛）——**热路径不许再查一次**，
    /// 所以一次 SQL 取全。
    #[allow(clippy::type_complexity)]
    pub async fn resolve_passkey_cached(
        &self,
        passkey: &str,
    ) -> Option<super::ratio_gate::Pku> {
        {
            let g = self.guard_read();
            if let Some((p, at)) = g.passkeys.get(passkey) {
                if at.elapsed() < PASSKEY_TTL {
                    return Some(*p);
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
        // 客户端还会把 tracker 标成错误、长时间不再重试）。prev 由改密接口写入；
        // 显式「重置密钥」不再写 prev（五轮 P2-6：那是安全处置动作）。
        let outcome = sqlx::query_as::<
            _,
            (i64, bool, bool, i32, i64, i64, i64, bool, f64, i64),
        >(
            "SELECT u.id, u.download_enabled, u.suspended, u.class_id, \
                    COALESCE(u.uploaded, 0)::bigint, \
                    COALESCE(u.downloaded, 0)::bigint, \
                    COALESCE( \
                      (EXTRACT(EPOCH FROM (now() - u.created_at))::bigint \
                          / 86400)::bigint, -1)::bigint, \
                    COALESCE(u.ratio_watch_until > now(), FALSE), \
                    COALESCE(uc.min_ratio, 0)::float8, \
                    COALESCE(uc.min_age_days, 0)::bigint \
             FROM user_by_passkey u \
             LEFT JOIN user_classes uc ON uc.id = u.class_id \
             WHERE u.passkey = $1 AND u.status < 2",
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
        let pku = row.map(
            |(id, de, su, cls, up, down, age, in_watch, cmin, cage)| {
                super::ratio_gate::Pku {
                    id,
                    download_enabled: de,
                    suspended: su,
                    class_id: cls,
                    uploaded: up,
                    downloaded: down,
                    age_days: age,
                    in_watch,
                    class_min_ratio: cmin,
                    class_age_days: cage,
                }
            },
        );
        let mut g = self.guard_write();
        if g.passkeys.len() + g.passkey_miss.len() >= PASSKEY_CACHE_CAP {
            g.passkeys.clear(); // 粗暴防膨胀：正常站点远达不到该量级
            g.passkey_miss.clear();
        }
        match &pku {
            Some(v) => {
                g.passkeys.insert(passkey.to_string(), (*v, Instant::now()));
            }
            None => {
                g.passkey_miss.insert(passkey.to_string(), Instant::now());
            }
        }
        pku
    }

    /// agent_rules 黑白名单判定（P0-7 交叉验证版；0351 加版本区间+优先级）：
    /// deny 命中即拒；allow 非空时须命中其一（UA 或 peer_id 任一）。
    /// 0351：规则先按 priority DESC 稳定排序（同 priority 保持加载序）；
    /// 带 version_min/version_max 的规则要求 UA 版本落在区间内（含边界），
    /// 版本提取失败的 UA 对带区间的规则不命中。工具函数在 agent_ver.rs。
    /// peer_id 传可读形式（lossy）以匹配 -XL0014- 等前缀。
    pub fn agent_blocked(
        &self,
        agent: Option<&str>,
        peer_id: &str,
    ) -> Option<String> {
        let mut rules = self.guard_read().agent_rules.clone()?;
        // priority DESC 稳定排序：sort_by 本身稳定，同 priority 保持原序
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        let a = agent.unwrap_or("");
        let ua_ver = ua_version(a);
        let hit = |r: &&AgentRule| {
            if !version_in_range(r, ua_ver) {
                return false;
            }
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

    /// 超频临时封禁命中检查（任务2）：passkey 拿到 user_id 之后调用，命中
    /// 返回 true。Redis 不可用时降级放行（与 rate_over 的降级纪律一致：
    /// 封禁是限流的加强项，不能反过来把正常 announce 拒掉）。
    pub async fn announce_user_banned(&self, user_id: i64) -> bool {
        if !ban_enabled(self.cfg.user_ban_secs) {
            return false;
        }
        let mut c = self.redis.clone();
        use redis::AsyncCommands;
        match c.exists::<_, i64>(&ban_key(user_id)).await {
            Ok(v) => v != 0,
            Err(_) => {
                self.metrics.redis_fallback.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    /// 超频临时封禁写入（任务2）：用户级限流命中时调用，键 `ban:ann:u:{id}`
    /// 值 1、TTL = ban_secs。Redis 不可用 / ban_secs=0（关闭）时跳过——
    /// 保留原限流拒绝语义，封禁只是加重项，不是必需路径。
    pub async fn ban_announce_user(&self, user_id: i64) {
        if !ban_enabled(self.cfg.user_ban_secs) {
            return;
        }
        let mut c = self.redis.clone();
        let key = ban_key(user_id);
        let r: Result<(), redis::RedisError> = redis::cmd("SET")
            .arg(&key)
            .arg("1")
            .arg("EX")
            .arg(self.cfg.user_ban_secs)
            .query_async(&mut c)
            .await;
        if r.is_err() {
            self.metrics.redis_fallback.fetch_add(1, Ordering::Relaxed);
        }
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

    /// `stopped` 用的独立 IP 桶（4× 宽阈）。状态转换被限流吞掉的代价不成比例：
    /// `snatches.seeding` 是粘滞的，只有 stopped 能清，drop 一发就等于让对端
    /// 永久滞留在 swarm 里，H&R 与做种收益判定全部跟着错。4× 仍给洪水留上限，
    /// 所以这里不是豁免限流、只是把额度按事件语义分账。
    pub async fn rate_limited_ip_stopped(
        &self,
        ip: &str,
    ) -> Option<&'static str> {
        let k = format!("rl:ann:ipstop:{ip}");
        self.rate_over(&k, (self.cfg.ip_per_min * 4).max(1))
            .await
            .then_some("announce 频率超限（IP），请稍后再试")
    }

    /// scrape 独立限流（审计 10-07 P2）：旧版与 announce 共用 `rl:ann:ip:` 桶，
    /// 一次全站轮询就把该 IP 的 announce 额度吃光，用户表现为「突然全站在报超限」。
    pub async fn rate_limited_scrape(&self, ip: &str) -> Option<&'static str> {
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
    pub async fn claim_event_window(
        &self,
        user_id: i64,
        info_hash: &str,
    ) -> bool {
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
