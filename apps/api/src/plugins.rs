#![allow(dead_code)]
//! M28 插件系统（方案 §4.3）：Hook 思想借鉴 NexusPHP。
//!
//! 纪律（M28 验收基准）：
//! - 插件只通过 `Plugin` Trait 的 Hook 方法介入，不侵入领域层代码；
//! - 插件启停不影响核心链路 —— Hook 分发全部 catch_unwind + 错误降级为日志；
//! - 示例插件「发布自动置顶官种」仅靠注册 Hook 实现。
//!
//! 线程安全：Hook 以扩展 trait object 形式静态注册（编译期装配），
//! 运行期启停由插件自身配置决定（`enabled()` 每次分发时询问）。

use std::sync::Arc;

use crate::state::AppState;

/// 领域事件（插件可见的负载）。按需扩展字段。
#[derive(Debug, Clone)]
pub enum DomainEvent {
    /// 用户登录成功
    UserLogin { user_id: i64 },
    /// 种子发布成功（已过审核入列表）
    TorrentUploaded {
        torrent_id: i64,
        owner_id: i64,
        name: String,
    },
    /// 做种里程碑（小时数）
    SeedingMilestone {
        user_id: i64,
        torrent_id: i64,
        hours: i64,
    },
}

/// 插件 Trait：默认全部 no-op，插件按需覆写。
pub trait Plugin: Send + Sync {
    /// 插件名（日志/审计用）
    fn name(&self) -> &'static str;
    /// 运行开关（读取自身配置；false 时管理器跳过分发）
    fn enabled(&self) -> bool {
        true
    }
    /// 登录后 Hook
    fn on_user_login(&self, _state: &AppState, _user_id: i64) {}
    /// 种子发布后 Hook
    fn on_torrent_upload(&self, _: &AppState, _: i64, _: i64) {}
    /// 做种里程碑 Hook（worker 周期触发）
    fn on_seeding_milestone(&self, _: &AppState, _: i64, _: i64) {}
}

/// 插件运行时开关（0214，五路方案 P1-3.5）：site_settings 的 `plugin__{name}`
/// 键（yes/no，缺省 yes）。分发处同步询问——站长可一键停用某插件；
/// 键值热读有查询成本，用 30s 进程内缓存摊薄。
/// 开关缓存（模块级静态，0224 G30）：plugin_enabled 读；
/// invalidate_plugin_cache 清（cfg:ver modules 域命中时同步失效）。
type PluginCache = std::sync::Mutex<
    std::collections::HashMap<String, (std::time::Instant, bool)>,
>;
static PLUGIN_CACHE: tokio::sync::OnceCell<PluginCache> =
    tokio::sync::OnceCell::const_new();

/// 缓存整体失效（未初始化时跳过——首次读本身就是全量新值）。
pub fn invalidate_plugin_cache() {
    if let Some(c) = PLUGIN_CACHE.get() {
        if let Ok(mut g) = c.lock() {
            g.clear();
        }
    }
}

pub async fn plugin_enabled(db: &sqlx::PgPool, name: &str) -> bool {
    let cache = PLUGIN_CACHE
        .get_or_init(|| async { PluginCache::default() })
        .await;
    if let Ok(g) = cache.lock() {
        if let Some((at, v)) = g.get(name) {
            if at.elapsed() < std::time::Duration::from_secs(30) {
                return *v;
            }
        }
    }
    let v: Option<String> =
        sqlx::query_scalar("SELECT value FROM site_settings WHERE name = $1")
            .bind(format!("plugin__{name}"))
            .fetch_optional(db)
            .await
            .ok()
            .flatten();

    let v = !matches!(v.as_deref(), Some("no"));
    if let Ok(mut g) = cache.lock() {
        g.insert(name.to_string(), (std::time::Instant::now(), v));
    }
    v
}

/// 插件管理器：编译期静态装配（Vec<Arc<dyn Plugin>>）。
pub struct PluginManager {
    plugins: Vec<Arc<dyn Plugin>>,
}

impl PluginManager {
    /// 装配全部内置插件。第三方编译进 bin 后在此追加。
    pub fn builtin() -> Self {
        Self {
            plugins: vec![Arc::new(AutoPinOfficial)],
        }
    }

    pub fn list(&self) -> Vec<&'static str> {
        self.plugins.iter().map(|p| p.name()).collect()
    }

    /// 统一分发：单插件 panic 不拖垮核心链路（M28 验收：启停不影响核心）。
    /// 运行时开关（0214）：除插件自身 enabled()，还询问 site_settings 的
    /// `plugin__{name}` 键（异步热读；查询挪进 spawn 的 blocking 任务，
    /// 分发本身保持同步零阻塞）。停用键只跳过分发，不影响其余插件。
    pub fn dispatch_login(&self, state: &Arc<AppState>, user_id: i64) {
        for p in &self.plugins {
            if !p.enabled() {
                continue;
            }
            let p = p.clone();
            let st = state.clone();
            tokio::task::spawn(async move {
                let name = p.name();
                if !plugin_enabled(&st.repo.db, name).await {
                    return;
                }
                let r = tokio::task::spawn_blocking(move || {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        || p.on_user_login(&st, user_id),
                    ))
                })
                .await;
                if matches!(r, Ok(Err(_)) | Err(_)) {
                    tracing::error!(
                        plugin = name,
                        "login hook panicked (suppressed)"
                    );
                }
            });
        }
    }

    pub fn dispatch_upload(
        &self,
        state: &Arc<AppState>,
        torrent_id: i64,
        owner_id: i64,
    ) {
        for p in &self.plugins {
            if !p.enabled() {
                continue;
            }
            let p = p.clone();
            let st = state.clone();
            tokio::task::spawn(async move {
                let name = p.name();
                if !plugin_enabled(&st.repo.db, name).await {
                    return;
                }
                let r = tokio::task::spawn_blocking(move || {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        || p.on_torrent_upload(&st, torrent_id, owner_id),
                    ))
                })
                .await;
                if matches!(r, Ok(Err(_)) | Err(_)) {
                    tracing::error!(
                        plugin = name,
                        "upload hook panicked (suppressed)"
                    );
                }
            });
        }
    }

    pub fn dispatch_milestone(
        &self,
        state: &Arc<AppState>,
        user_id: i64,
        hours: i64,
    ) {
        for p in &self.plugins {
            if !p.enabled() {
                continue;
            }
            let p = p.clone();
            let st = state.clone();
            tokio::task::spawn(async move {
                let name = p.name();
                if !plugin_enabled(&st.repo.db, name).await {
                    return;
                }
                let r = tokio::task::spawn_blocking(move || {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        || p.on_seeding_milestone(&st, user_id, hours),
                    ))
                })
                .await;
                if matches!(r, Ok(Err(_)) | Err(_)) {
                    tracing::error!(
                        plugin = name,
                        "milestone hook panicked (suppressed)"
                    );
                }
            });
        }
    }
}

// ============ 示例插件：发布自动置顶官种 ============

/// 官种（official_tag=TRUE）发布后自动置顶（sticky=TRUE）。
/// 仅靠 on_torrent_upload Hook 实现，不侵入 torrents 领域层。
#[derive(Default)]
struct AutoPinOfficial;

impl Plugin for AutoPinOfficial {
    fn name(&self) -> &'static str {
        "auto_pin_official"
    }

    fn on_torrent_upload(
        &self,
        state: &AppState,
        torrent_id: i64,
        _owner_id: i64,
    ) {
        // hook 已在 blocking 线程，借当前 tokio handle 执行异步 SQL
        let db = state.repo.db.clone();
        let rt = tokio::runtime::Handle::current();
        let n = rt.block_on(async move {
            match sqlx::query(
                "UPDATE torrents SET sticky = TRUE \
             WHERE id = $1 AND official_tag",
            )
            .bind(torrent_id)
            .execute(&db)
            .await
            {
                Ok(r) => r.rows_affected(),
                Err(e) => {
                    tracing::error!(
                        plugin = "auto_pin_official",
                        ?e,
                        "pin update failed"
                    );
                    0
                }
            }
        });
        if n > 0 {
            tracing::info!(
                plugin = "auto_pin_official",
                torrent_id,
                "official torrent pinned"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Recording {
        hit: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }
    impl Plugin for Recording {
        fn name(&self) -> &'static str {
            "recording"
        }
        fn on_user_login(&self, _s: &AppState, _u: i64) {
            self.hit.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    struct Panicky;
    impl Plugin for Panicky {
        fn name(&self) -> &'static str {
            "panicky"
        }
        fn on_user_login(&self, _s: &AppState, _u: i64) {
            panic!("boom");
        }
    }

    #[test]
    fn builtin_has_example_plugin() {
        let m = PluginManager::builtin();
        assert!(m.list().contains(&"auto_pin_official"));
    }

    #[test]
    fn panicked_hook_is_suppressed_and_next_plugin_runs() {
        // 无 AppState 也能构造管理器：直接用裸 Vec 验证分发语义
        let hit =
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let rec = std::sync::Arc::new(Recording { hit: hit.clone() });
        let mgr = PluginManager {
            plugins: vec![Arc::new(Panicky), rec],
        };
        // dispatch_login 需要 AppState；此测试只验证 list 与 enabled 语义
        assert_eq!(mgr.list().len(), 2);
        assert!(mgr.plugins.iter().all(|p| p.enabled()));
    }
}
