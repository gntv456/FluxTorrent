//! 种子白名单与 swarm 元数据读口（审计 2026-10-08 五轮自 guard_refresh.rs
//! 拆出，300 行门禁）。刷新链路在 guard_refresh.rs，缓存结构与段封禁在
//! guard_store.rs，未注册负缓存在 guard_miss.rs——这里只回答两件事：
//! 「这颗种子能不能 announce」与「它的大小 / 完成数 / 发布者 / 审核态」。

use super::guard_store;
use super::helpers::TrackerState;

impl TrackerState {
    /// 种子白名单判定（P0-2）：info_hash 是否已在站内注册且可 announce。
    /// 命中快照 O(1)；未命中先查负缓存（审计 10-07 P3：旧版每次 miss 都
    /// 直查 PG，随机 hash 洪水把白名单本身变成 DB 放大器），再直查兜底新发种窗口。
    ///
    /// 解码纪律（10-07 P3-1）：旧写法 `SELECT 1` 配 `query_scalar::<_, i64>`
    /// 命中行必因 INT4↔INT8 失配报错，被 `.ok()` 吞成「放行」，于是这条兜底
    /// 路径从未真正确认过任何种子。现在按带类型的元组取，并把
    /// **查询失败**与**确实没这颗种**分开处理（五轮 2026-10-08）：
    /// 旧写法 `.ok().flatten()` 把两者混成 None ⇒ 既按**拒绝**处理（与本函数
    /// 注释承诺的 fail-open 相反，一次 DB 抖动就打断新种的 announce），
    /// 又把它写进 60s 负缓存（抖动期间反复自我加固这个误判）。
    /// 现在：抖动 ⇒ 本次放行、不写负缓存、留 warn。
    pub(crate) async fn torrent_registered(&self, info_hash: &str) -> bool {
        {
            let g = self.guard_read();
            if let Some(set) = &g.known_hashes {
                if set.contains(info_hash) {
                    return true;
                }
            }
        }
        if super::guard_miss::miss_seen(info_hash) {
            return false;
        }
        let row: Result<Option<(i64, i32, i64, i16)>, sqlx::Error> =
            sqlx::query_as(
                "SELECT COALESCE(size, 0), COALESCE(times_completed, 0), \
                        COALESCE(owner_id, 0), approval_status \
                 FROM torrents WHERE (info_hash = $1 OR raw_info_hash = $1) \
                   AND approval_status IN (0, 1) LIMIT 1",
            )
            .bind(info_hash)
            .fetch_optional(&self.db)
            .await;
        match row {
            Ok(Some((size, completed, owner_id, approval))) => {
                if let Ok(mut w) = guard_store::meta_map().write() {
                    w.insert(
                        info_hash.to_string(),
                        guard_store::SwarmMeta {
                            size,
                            completed: completed as i64,
                            owner_id,
                            approval,
                        },
                    );
                }
                true
            }
            Ok(None) => {
                super::guard_miss::remember_miss(info_hash);
                false
            }
            Err(e) => {
                tracing::warn!(
                    %e, %info_hash,
                    "种子白名单兜底查询失败，本次按放行处理且不写负缓存"
                );
                true
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
        guard_store::meta_of(info_hash)
            .map(|m| m.completed.max(0) as usize)
            .unwrap_or_default()
    }

    /// 该种子的发布者与审核态（待审种子准入策略用）。
    pub(crate) fn swarm_owner(&self, info_hash: &str) -> Option<(i64, i16)> {
        guard_store::meta_of(info_hash).map(|m| (m.owner_id, m.approval))
    }

    /// ip_bans 命中 → 封禁理由。精确表之外再查段表（审计 10-07 P2）。
    pub fn ip_banned(&self, ip: &str) -> Option<String> {
        if let Some(r) = self.guard_read().ip_bans.get(ip).cloned() {
            return Some(r);
        }
        guard_store::ban_hit(ip)
    }
}
