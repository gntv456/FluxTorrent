//! 防护缓存刷新与种子白名单（审计 10-07 自 guard.rs 拆出，300 行门禁）。
//! passkey 缓存与限流窗口在 guard.rs。

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use super::helpers::{AgentRule, TrackerState, GUARD_REFRESH};

/// 刷新单飞锁（见 `refresh_guard` 内的说明）。进程级而非 `GuardInner` 字段：
/// 后者要改 main.rs 的初始化字面量，而 main.rs 正被另一路改动。
static REFRESHING: AtomicBool = AtomicBool::new(false);

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
        // 单飞（五轮 2026-10-08）：60s 一到，**所有**在途请求都看到 stale，
        // 旧写法让它们各自跑一遍全量重拉——白名单那条是一次 10 万行的 UNION
        // 加 20 MB 字符串集合，恰好在流量最高的时刻把 PG 变成放大器（惊群）。
        // 抢不到这一格的请求直接吃旧快照返回：60s 的缓存语义本来就允许短暂陈旧。
        // RAII 复位而不是手写 store(false)：actix 会在客户端断连时 drop 掉这个
        // future，任何「返回前不复位」的路径都等于让防护缓存从此不再刷新。
        struct Refreshing;
        impl Drop for Refreshing {
            fn drop(&mut self) {
                REFRESHING.store(false, std::sync::atomic::Ordering::Relaxed);
            }
        }
        if REFRESHING.swap(true, Ordering::Relaxed) {
            return;
        }
        let _g = Refreshing;
        self.force_refresh.store(false, Ordering::Relaxed);
        // ip::text 而非 host(ip)：后者剥掉掩码，段封禁会静默退化成单 host 匹配
        let bans: Option<Vec<(String, String)>> = sqlx::query_as(
            "SELECT ip::text, COALESCE(reason, '') FROM ip_bans",
        )
        .fetch_all(&self.db)
        .await
        .ok();
        let rules: Option<Vec<AgentRule>> =
            sqlx::query_as::<_, (String, String, String)>(
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
                                    tracing::warn!(
                                        %agent, %e,
                                        "agent_rules 正则无效，规则跳过"
                                    );
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
                                    tracing::warn!(
                                        %peer, %e,
                                        "agent_rules peer_id 正则无效，规则跳过"
                                    );
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
        // 「查询失败保留旧值」是本函数自述的纪律，五轮实测发现两处把它违反了：
        // interval 与 pending_policy 用 `.ok().flatten().unwrap_or(缺省)` 的写法，
        // 让**瞬时 DB 抖动**与**设定本来就不存在**长得一模一样，都被当成
        // 「按缺省值覆盖」。前者会把站长设定的 interval（连带 TTL / 合并窗长度）
        // 与准入档位静默重置回默认——一次抖动一次改策略，且面板上看不出来。
        // 口径：Ok(Some(v))=按值生效；Ok(None)=设定缺失，用缺省；Err=保留旧值 + 告警。
        let interval_res = sqlx::query_scalar::<_, String>(
            "SELECT value FROM site_settings WHERE name = 'announce_interval'",
        )
        .fetch_optional(&self.db)
        .await;
        let interval = match interval_res {
            Ok(Some(v)) => v
                .parse::<i64>()
                .map(|n| n.clamp(60, 86400))
                .unwrap_or(self.cfg.default_interval),
            Ok(None) => self.cfg.default_interval,
            Err(e) => {
                tracing::warn!(%e, "announce_interval 读取失败，保留旧值");
                self.guard_read().announce_interval
            }
        };
        // 待审种子准入策略（审计 10-07 P1-2）：缺失/非法一律按 self_seed_only(1)
        let policy_res = sqlx::query_scalar::<_, String>(
            "SELECT value FROM site_settings WHERE name = $1",
        )
        .bind("announce_pending_policy")
        .fetch_optional(&self.db)
        .await;
        match policy_res {
            Ok(Some(v)) => {
                super::guard_store::set_pending_policy(match v.trim() {
                    "allow_all" => 0,
                    "owner_only" => 2,
                    _ => 1,
                })
            }
            // 设定行不存在（新装站/被删）：回到安全默认档
            Ok(None) => super::guard_store::set_pending_policy(1),
            Err(e) => tracing::warn!(
                %e,
                "announce_pending_policy 读取失败，保留旧档位"
            ),
        }
        // peer 存活 TTL 随 interval 伸缩（审计 10-06 第 4 条）：leecher 曾硬编码
        // 90s，interval=1800s 下两次 announce 之间即被除名，在线数长期偏低。
        crate::peers::set_interval_secs(interval);
        // 种子白名单（P0-2）：全量 info_hash 双口径（规范化 + 原始字节），
        // 顺带带出大小/完成数/发布者/审核态供白名单兜底与 scrape 完成数用。
        // 十万级字符串 + 定长元数据 ≈ 20MB 内，60s 全量重拉可接受；
        // 拉取失败保留旧快照（与 ip_bans 同纪律）。
        let meta_rows: Option<Vec<(String, i64, i32, i64, i16)>> =
            sqlx::query_as(
                "SELECT h, COALESCE(size, 0), COALESCE(times_completed, 0), \
                    COALESCE(owner_id, 0), approval_status FROM ( \
               SELECT info_hash AS h, size, times_completed, \
                      owner_id, approval_status \
                 FROM torrents WHERE approval_status IN (0, 1) \
               UNION \
               SELECT raw_info_hash AS h, size, times_completed, \
                      owner_id, approval_status \
                 FROM torrents WHERE approval_status IN (0, 1) \
                   AND raw_info_hash IS NOT NULL) t",
            )
            .fetch_all(&self.db)
            .await
            .ok();

        let mut g = self.guard_write();
        if let Some(b) = bans {
            // 带掩码的进段表，精确项留在原表维持既有语义（含坏格式，避免
            // 一次解析失败就让这条封禁整体失效）
            g.ip_bans = super::guard_store::load_bans(&b).into_iter().collect();
        }
        if let Some(r) = rules {
            g.agent_rules = Some(r);
        }
        let hashes: Option<std::collections::HashSet<String>> =
            meta_rows.map(|rows| {
                let mut set = std::collections::HashSet::new();
                let mut meta = std::collections::HashMap::new();
                for (h, size, completed, owner_id, approval) in rows {
                    set.insert(h.clone());
                    meta.insert(
                        h,
                        super::guard_store::SwarmMeta {
                            size,
                            completed: completed as i64,
                            owner_id,
                            approval,
                        },
                    );
                }
                if let Ok(mut w) = super::guard_store::meta_map().write() {
                    *w = meta;
                }
                set
            });
        if let Some(h) = hashes {
            g.known_hashes = Some(h);
        }
        g.announce_interval = interval;
        g.refreshed_at = Instant::now();
        // 刷新即清未注册负缓存：新发种与审核态翻转最迟一个刷新周期内生效
        super::guard_miss::clear_miss();
    }
}
