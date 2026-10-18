//! 分发面（自 plugins.rs 抽出，守 300 行门禁）。
//!
//! 四个 dispatch_* 彼此同构，故口径只有一处：
//!   遍历插件 -> 查 site_settings 开关 -> `ensure_isolation` 建 DB 层
//!   隔离（0378）-> `spawn_blocking` 执行 hook -> `catch_unwind` 兜底。
//!
//! 【为什么隔离在 blocking 之前做】hook 是同步的（内部 `block_on`），
//! 建角色要 await，所以必须在进 blocking 之前把 `Isolation` 备好带进去。

use std::sync::Arc;

use crate::plugin_ctx::PluginCtx;
use crate::plugins::{Plugin, PluginManager};
use crate::state::AppState;

impl PluginManager {
    /// 统一分发：单插件 panic 不拖垮核心链路（M28 验收：启停不影响核心）。
    /// 运行时开关（0214）：除插件自身 enabled()，还询问 site_settings 的
    /// `plugin__{name}` 键（异步热读；查询挪进 spawn 的 blocking 任务，
    /// 分发本身保持同步零阻塞）。停用键只跳过分发，不影响其余插件。
    pub fn dispatch_login(&self, state: &Arc<AppState>, user_id: i64) {
        for (p, caps) in &self.plugins {
            if !p.enabled() {
                continue;
            }
            let p = p.clone();
            let caps = caps.clone();
            let st = state.clone();
            tokio::task::spawn(async move {
                let name = p.name();
                let on =
                    crate::plugin_ctx::plugin_enabled(&st.repo.db, name).await;
                if !on {
                    return;
                }
                // DB 层隔离（0378）：hook 同步，故在此 await 备好角色，
                // 再带进 blocking 闭包。无 CREATEROLE 时自动降级。
                let iso = crate::plugin_sql::ensure_isolation(
                    &st.repo.db,
                    name,
                    &caps,
                )
                .await;
                let r = tokio::task::spawn_blocking(move || {
                    let ctx = PluginCtx::new(&st.repo.db, &caps, name, &iso);
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        || p.on_user_login(&ctx, user_id),
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
        for (p, caps) in &self.plugins {
            if !p.enabled() {
                continue;
            }
            let p = p.clone();
            let caps = caps.clone();
            let st = state.clone();
            tokio::task::spawn(async move {
                let name = p.name();
                if !crate::plugin_ctx::plugin_enabled(&st.repo.db, name).await {
                    return;
                }
                let iso = crate::plugin_sql::ensure_isolation(
                    &st.repo.db,
                    name,
                    &caps,
                )
                .await;
                let r = tokio::task::spawn_blocking(move || {
                    let ctx = PluginCtx::new(&st.repo.db, &caps, name, &iso);
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        || p.on_torrent_upload(&ctx, torrent_id, owner_id),
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

    /// pool 版上传分发（0374 事件总线的消费面）。
    ///
    /// 与 `dispatch_upload` 的差别只有一个：不要 `&AppState`。
    /// 消费循环只需要 DB 与插件表，造 AppState 会把 Redis/JWT 一起拖
    /// 进来（后台任务不该依赖 Redis 可达）。
    pub fn dispatch_upload_pool(
        &self,
        pool: &sqlx::PgPool,
        torrent_id: i64,
        owner_id: i64,
    ) {
        for (p, caps) in &self.plugins {
            if !p.enabled() {
                continue;
            }
            let p = p.clone();
            let caps = caps.clone();
            let pool = pool.clone();
            tokio::task::spawn(async move {
                let name = p.name();
                let on = crate::plugin_ctx::plugin_enabled(&pool, name).await;
                if !on {
                    return;
                }
                let iso =
                    crate::plugin_sql::ensure_isolation(&pool, name, &caps)
                        .await;
                let r = tokio::task::spawn_blocking(move || {
                    let ctx = PluginCtx::new(&pool, &caps, name, &iso);
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        || p.on_torrent_upload(&ctx, torrent_id, owner_id),
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
        for (p, caps) in &self.plugins {
            if !p.enabled() {
                continue;
            }
            let p = p.clone();
            let caps = caps.clone();
            let st = state.clone();
            tokio::task::spawn(async move {
                let name = p.name();
                if !crate::plugin_ctx::plugin_enabled(&st.repo.db, name).await {
                    return;
                }
                let iso = crate::plugin_sql::ensure_isolation(
                    &st.repo.db,
                    name,
                    &caps,
                )
                .await;
                let r = tokio::task::spawn_blocking(move || {
                    let ctx = PluginCtx::new(&st.repo.db, &caps, name, &iso);
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        || p.on_seeding_milestone(&ctx, user_id, hours),
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
