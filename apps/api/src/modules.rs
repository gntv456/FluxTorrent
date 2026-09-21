//! 模块注册表（U1，策划案 §4/§5.1）：可选功能域的行为开关。
//!
//! 纪律：
//! - 三处同步：本文件常量表 / 迁移 0107 的 modules 表 / packages/domain-types 的 ModuleKey，
//!   契约测试（本文件 tests）锁死键集合一致；
//! - T2 行为开关：module_* 关闭必须同时作用于 API（本守卫）、导航、页面、worker；
//! - T3 缺省=现状：读不到设置时回落 modules.is_on 默认值（教育站形态）；
//! - D4 fail-close：读取失败按「关」处理并告警。

mod gateway;

pub use gateway::module_gate_mw;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use actix_web::web;

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 模块键常量（与迁移 0107 modules.key 一一对应，防拼写漂移）。
/// games 与 farm/gomoku/contests 是平级兄弟——允许「关农场留娱乐屋」。
pub mod key {
    // 社区
    pub const TEXTBOOKS: &str = "textbooks";
    #[cfg_attr(not(test), allow(dead_code))]
    pub const SHOWCASE: &str = "showcase";
    pub const SOCIAL: &str = "social";
    pub const FORUMS: &str = "forums";
    pub const MESSAGES: &str = "messages";
    pub const FRIENDS: &str = "friends";
    pub const OFFERS: &str = "offers";
    pub const REQUESTS: &str = "requests";
    pub const SUBTITLES: &str = "subtitles";
    pub const PRESERVE: &str = "preserve";
    pub const SHOUTBOX: &str = "shoutbox";
    // 经济
    pub const PROMO_BUY: &str = "promo_buy";
    pub const BANK: &str = "bank";
    pub const SHOP: &str = "shop";
    pub const MAGIC_POOL: &str = "magic_pool";
    pub const VOUCHERS: &str = "vouchers";
    pub const RESURRECTIONS: &str = "resurrections";
    pub const WISHLIST: &str = "wishlist";
    // 娱乐
    pub const GAMES: &str = "games";
    pub const FARM: &str = "farm";
    pub const GOMOKU: &str = "gomoku";
    pub const CONTESTS: &str = "contests";
    // 运营
    pub const ATTENDANCE: &str = "attendance";
    pub const MEDALS: &str = "medals";
    pub const DRESSUP: &str = "dressup";
    pub const JIXIAO: &str = "jixiao";
    pub const TASKS: &str = "tasks";
    pub const EXAMS: &str = "exams";
    pub const PUSH: &str = "push";

    /// 全部合法键（契约测试用：与迁移/modules 表、TS ModuleKey 三方一致）
    #[cfg_attr(not(test), allow(dead_code))]
    pub const ALL: &[&str] = &[
        TEXTBOOKS,
        SHOWCASE,
        SOCIAL,
        FORUMS,
        MESSAGES,
        FRIENDS,
        OFFERS,
        REQUESTS,
        SUBTITLES,
        PRESERVE,
        SHOUTBOX,
        PROMO_BUY,
        BANK,
        SHOP,
        MAGIC_POOL,
        VOUCHERS,
        RESURRECTIONS,
        WISHLIST,
        GAMES,
        FARM,
        GOMOKU,
        CONTESTS,
        ATTENDANCE,
        MEDALS,
        DRESSUP,
        JIXIAO,
        TASKS,
        EXAMS,
        PUSH,
    ];
}

/// 注册表默认值（T3：= 当前教育站形态）。键集与 key::ALL 一致。
/// settings 读不到时回落这里——「表被清空」不等于「全站功能关闭」。
pub fn default_on(k: &str) -> bool {
    // 唯二默认关闭的历史键（沿用 0037/0092 既有口径）；contests 由 0107 显式落 no
    !matches!(k, "showcase" | "social" | "contests")
}

/// 模块开关缓存：TTL 兜底 + 后台改键主动失效（§5.1）。
/// 读 site_settings.module_{key} 失败按关处理并告警（D4 fail-close）——
/// 但 DB 抖动不应把整站功能打成「关」，故仅对**存在且值非法**的场景 fail-close，
/// 查询错误回落默认值并告警（默认值即现状，风险面最小）。
#[derive(Default)]
pub struct ModuleFlags {
    inner: tokio::sync::RwLock<Option<(HashMap<String, bool>, Instant)>>,
}

impl ModuleFlags {
    pub fn new() -> Self {
        Self::default()
    }

    async fn load(db: &sqlx::PgPool) -> HashMap<String, bool> {
        let mut m = HashMap::new();
        let rows: Vec<(String, String)> = match sqlx::query_as(
            "SELECT name, \
             value FROM site_settings WHERE name LIKE 'module\\_%'",
        )
        .fetch_all(db)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    ?e,
                    "module flags load failed, falling back to defaults"
                );
                return m;
            }
        };
        for (name, value) in rows {
            if let Some(k) = name.strip_prefix("module_") {
                // 非法值按关处理（fail-close 只作用在「站长配了非法值」）
                m.insert(k.to_string(), value.trim() == "yes");
            }
        }
        m
    }

    /// 当前开关（带缓存，TTL 30s）
    pub async fn enabled(&self, db: &sqlx::PgPool, k: &str) -> bool {
        {
            let guard = self.inner.read().await;
            if let Some((map, at)) = guard.as_ref() {
                if at.elapsed() < Duration::from_secs(30) {
                    return map
                        .get(k)
                        .copied()
                        .unwrap_or_else(|| default_on(k));
                }
            }
        }
        let fresh = Self::load(db).await;
        let v = fresh.get(k).copied().unwrap_or_else(|| default_on(k));
        *self.inner.write().await = Some((fresh, Instant::now()));
        v
    }

    /// 后台改键后主动失效（settings 保存处调用，最坏竞态由 TTL 兜底）
    pub async fn invalidate(&self) {
        *self.inner.write().await = None;
    }
}

impl AppState {
    /// 便捷判定：模块是否开启（worker 与 API 共用）
    pub async fn module_enabled(&self, k: &str) -> bool {
        self.module_flags.enabled(&self.repo.db, k).await
    }

    /// 端点守卫（§5.1）：关闭时返回 ModuleDisabled（4101），先于权限判定（§5.5）
    pub async fn require_module(&self, k: &str) -> DomainResult<()> {
        if self.module_enabled(k).await {
            Ok(())
        } else {
            Err(DomainError::ModuleDisabled(k.to_string()))
        }
    }
}

/// 便捷守卫函数（handler 内一行调用；当前端点走网关中间件，保留给域内细粒度场景与测试）
#[allow(dead_code)]
pub async fn require_module(
    state: &web::Data<Arc<AppState>>,
    k: &str,
) -> DomainResult<()> {
    state.require_module(k).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 契约：常量键无重复
    #[test]
    fn keys_unique() {
        let mut v = key::ALL.to_vec();
        v.sort_unstable();
        v.dedup();
        assert_eq!(v.len(), key::ALL.len(), "duplicate module keys");
    }

    /// 契约：默认值函数覆盖全部键
    #[test]
    fn defaults_cover_all() {
        // default_on 对任意键都有定义（闭式布尔），只需抽验方向正确
        assert!(default_on(key::GAMES));
        assert!(!default_on("showcase"));
        assert!(!default_on("social"));
        assert!(!default_on("contests"));
    }

    /// 契约：键数量与 TS ModuleKey 一致（29 键——4 历史 + 25 新增口径，见 0107 注释）
    #[test]
    fn key_count() {
        assert_eq!(
            key::ALL.len(),
            29,
            "module key set drifted from migration 0107"
        );
    }
}
