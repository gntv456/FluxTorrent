//! 模块注册表（U1，策划案 §4/§5.1）：可选功能域的行为开关。
//!
//! 纪律：
//! - 三处同步：本文件 `key::ALL` / `packages/domain-types` 的 MODULE_KEYS /
//!   DB modules 表，Rust 侧 tests 锁键数，跨语言比对由
//!   `scripts/module_keys_guard.mjs` 承担
//!   （0197 加 invites 时只改了 Rust、TS 漏改而无人报错，才补的这道门）；
//! - T2 行为开关：module_* 关闭必须同时作用于 API（本守卫）、导航、页面、worker；
//! - T3 缺省=general 中立矩阵（0178 翻转）：读不到 site_settings.module_<key> 时
//!   回落本文件 `default_on()`——**开关真值只有 site_settings 一处**（0200 起
//!   modules.is_on 已删，别再去找第二份）；
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
    pub const INVITES: &str = "invites";

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
        INVITES,
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

/// 注册表默认值（0178 翻转：= general 中立矩阵）。键集与 key::ALL 一致。
/// settings 读不到时回落这里——裸库/漏键实例得到中立通用站形态，
/// 不再呈现教育站全开（缺省形态站型中立纪律，二审 R1）。
pub fn default_on(k: &str) -> bool {
    !matches!(
        k,
        "textbooks"
            | "showcase"
            | "social"
            | "farm"
            | "gomoku"
            | "contests"
            | "jixiao"
            | "exams"
    )
}

/// 自建产物（自定义页面 / 用户自定义字段 / 菜单项）挂模块键时的校验：
/// NULL = 不挂（恒可见）；非空必须存在于注册表，否则宁拒不悬——写错一个字母
/// 会让产物在后台看得见、前台永久消失，且没有任何线索。
pub async fn require_known_module(
    db: &sqlx::PgPool,
    k: Option<&str>,
) -> crate::errors::DomainResult<()> {
    let Some(k) = k.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(());
    };
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM modules WHERE key = $1)",
    )
    .bind(k)
    .fetch_one(db)
    .await
    .map_err(|e| crate::errors::DomainError::Internal(e.into()))?;
    if !exists {
        return Err(crate::errors::DomainError::Validation(format!(
            "未知模块键 {k}（可用键见后台「模块开关」）"
        )));
    }
    Ok(())
}

/// 「挂了模块键的行是否该出现」的 SQL 判据（同 menus_public 的 T3 语义：
/// 未配置键按关处理）。集中一处，避免每个读路径各写一份而悄悄漂移。
/// `alias` 传表名或别名；传空串则用裸列名。
pub fn module_on_sql(alias: &str) -> String {
    let p = if alias.is_empty() {
        String::new()
    } else {
        format!("{alias}.")
    };
    format!(
        "({p}module_key IS NULL OR COALESCE((SELECT value = 'yes' \
         FROM site_settings sg WHERE sg.name = 'module_' || \
         {p}module_key), false))"
    )
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

    /// 契约：默认值函数覆盖全部键（0178 翻转后 = general 中立矩阵）
    #[test]
    fn defaults_cover_all() {
        // default_on 对任意键都有定义（闭式布尔），只需抽验方向正确
        assert!(default_on(key::GAMES));
        assert!(default_on(key::SUBTITLES));
        assert!(default_on(key::FORUMS));
        // 教育考核与重度娱乐缺省关（中立形态）
        for k in [
            "textbooks",
            "jixiao",
            "exams",
            "farm",
            "gomoku",
            "contests",
            "showcase",
            "social",
        ] {
            assert!(!default_on(k), "default_on({k}) 应为 false");
        }
    }

    /// 契约：键数量与 TS ModuleKey 一致（29 键——4 历史 + 25 新增口径，见 0107 注释）
    #[test]
    fn key_count() {
        assert_eq!(
            key::ALL.len(),
            30,
            "module key set drifted from migration 0107（+0197 invites）"
        );
    }
}
