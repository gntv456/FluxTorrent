//! 防护缓存刷新与种子白名单（审计 10-07 自 guard.rs 拆出，300 行门禁）。
//! passkey 缓存与限流窗口在 guard.rs。

use std::sync::atomic::Ordering;
use std::time::Instant;

use super::helpers::{AgentRule, TrackerState, GUARD_REFRESH};

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
        // ip::text 而非 host(ip)：后者剥掉掩码，段封禁会静默退化成单 host 匹配
        let bans: Option<Vec<(String, String)>> = sqlx::query_as(
            "SELECT ip::text, COALESCE(reason, '') FROM ip_bans",
        )
        .fetch_all(&self.db)
        .await
        .ok();
        let rules: Option<Vec<AgentRule>> = sqlx::query_as::<
            _,
            (String, String, String),
        >(
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
            g.ip_bans = super::guard_store::load_bans(&b)
                .into_iter()
                .collect();
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
        super::guard_store::clear_miss();
    }

    /// 种子白名单判定（P0-2）：info_hash 是否已在站内注册且可 announce。
    /// 命中快照 O(1)；未命中先查 60s 负缓存（审计 10-07 P3：旧版每次 miss 都
    /// 直查 PG，随机 hash 洪水把白名单本身变成 DB 放大器），再直查兜底新发种窗口。
    /// 查询失败仍 fail-open（DB 抖动不该打断全站 announce），但**不再被解码错吞掉**：
    /// 旧写法 `SELECT 1` 配 `query_scalar::<_, i64>` 命中行必因 INT4↔INT8 失配报错，
    /// 被 `.ok()` 吞成「放行」，于是这条兜底路径从未真正确认过任何种子。
    pub(crate) async fn torrent_registered(&self, info_hash: &str) -> bool {
        {
            let g = self.guard_read();
            if let Some(set) = &g.known_hashes {
                if set.contains(info_hash) {
                    return true;
                }
            }
        }
        if super::guard_store::miss_seen(info_hash) {
            return false;
        }
        let row: Option<(i64, i32, i64, i16)> = sqlx::query_as(
            "SELECT COALESCE(size, 0), COALESCE(times_completed, 0), \
                    COALESCE(owner_id, 0), approval_status \
             FROM torrents WHERE (info_hash = $1 OR raw_info_hash = $1) \
               AND approval_status IN (0, 1) LIMIT 1",
        )
        .bind(info_hash)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| {
            tracing::warn!(%e, "种子白名单兜底查询失败，本次按放行处理")
        })
        .ok()
        .flatten();
        match row {
            Some((size, completed, owner_id, approval)) => {
                if let Ok(mut w) = super::guard_store::meta_map().write() {
                    w.insert(
                        info_hash.to_string(),
                        super::guard_store::SwarmMeta {
                            size,
                            completed: completed as i64,
                            owner_id,
                            approval,
                        },
                    );
                }
                true
            }
            None => {
                super::guard_store::remember_miss(info_hash);
                false
            }
        }
    }

    /// scrape 用白名单判定（审计 10-06 第 3 条）：scrape 一次可带几十个
    /// info_hash，逐 miss 直查 PG 等于把 scrape 变成 DB 放大器——只认 60s
    /// 全量快照（未命中即未注册，计数按 0 回）。快照尚未完成首次加载时
    /// 退化为直查（仅启动最初一瞬；scrape 链路先 refresh_guard 预热）。
    pub(crate) async fn torrent_registered_scrape(
        &self,
        info_hash: &str,
    ) -> bool {
        if let Some(set) = &self.guard_read().known_hashes {
            return set.contains(info_hash);
        }
        self.torrent_registered(info_hash).await
    }

    /// 该种子的完成数（BEP3 `downloaded`）。旧实现 announce/scrape 都硬写 0，
    /// 客户端「完成」列永远空——而 `torrents.times_completed` 早就是权威值。
    pub(crate) fn scrape_completed(&self, info_hash: &str) -> usize {
        super::guard_store::meta_of(info_hash)
            .map(|m| m.completed.max(0) as usize)
            .unwrap_or_default()
    }

    /// 该种子的大小（`left > size` 假 announce 判据的数据源）。
    /// 消费点在 announce.rs——那文件此刻正被另一路改动，故本批先留读口
    /// （同 table.rs `all_unreachable` 的预留口径）。
    #[allow(dead_code)] // 预留：left>size 假 announce 与待审准入都读它
    pub(crate) fn swarm_size(&self, info_hash: &str) -> Option<i64> {
        super::guard_store::meta_of(info_hash).map(|m| m.size)
    }

    /// 该种子的发布者与审核态（待审种子准入策略用，下一批接进 announce）。
    #[allow(dead_code)] // 预留：announce_pending_policy
    pub(crate) fn swarm_owner(&self, info_hash: &str) -> Option<(i64, i16)> {
        super::guard_store::meta_of(info_hash).map(|m| (m.owner_id, m.approval))
    }

    /// ip_bans 命中 → 封禁理由。精确表之外再查段表（审计 10-07 P2）。
    pub fn ip_banned(&self, ip: &str) -> Option<String> {
        if let Some(r) = self.guard_read().ip_bans.get(ip).cloned() {
            return Some(r);
        }
        super::guard_store::ban_hit(ip)
    }
}
