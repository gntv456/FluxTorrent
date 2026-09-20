//! 模块注册表（U1，策划案 §4/§5.1）：可选功能域的行为开关。
//!
//! 纪律：
//! - 三处同步：本文件常量表 / 迁移 0107 的 modules 表 / packages/domain-types 的 ModuleKey，
//!   契约测试（本文件 tests）锁死键集合一致；
//! - T2 行为开关：module_* 关闭必须同时作用于 API（本守卫）、导航、页面、worker；
//! - T3 缺省=现状：读不到设置时回落 modules.is_on 默认值（教育站形态）；
//! - D4 fail-close：读取失败按「关」处理并告警。

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
            "SELECT name, value FROM site_settings WHERE name LIKE 'module\\_%'",
        )
        .fetch_all(db)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(?e, "module flags load failed, falling back to defaults");
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

// ============ API 模块网关（§5.2 的收敛实现：路径前缀 → 模块映射） ============
//
// 策划案允许逐 handler 加守卫，但 460+ 端点逐个改既易漏又难评审（风险表第 2 条）。
// 中间件按「路径前缀表」一次性拦截全部可选模块端点：
// - 映射表遗漏的端点 = 未登记，本就不该被开关影响（核心层）；
// - 免检名单放横切端点（管理端/健康检查/兼容端点等）——管理后台始终可访问，
//   站长要启用模块得先能进设置中心（§5.5：管理员不绕过开关，但能到达开关本身）。

/// 路径前缀 → 模块键。命中即按该模块开关拦截。
/// 维护纪律 D2：新模块端点必须登记在此（契约测试无法枚举路由，靠评审清单把关）。
fn route_module(path: &str) -> Option<&'static str> {
    // 较长前缀先匹配（/bank/demand 也归 bank）
    const TABLE: &[(&str, &str)] = &[
        // 娱乐
        ("/api/v1/games", key::GAMES),
        ("/api/v1/farm", key::FARM),
        ("/api/v1/gomoku", key::GOMOKU),
        ("/api/v1/contests", key::CONTESTS),
        ("/api/v1/fun", key::GAMES), // 趣味投票/娱乐屋条目归 games 口径
        // 经济
        ("/api/v1/bank", key::BANK),
        ("/api/v1/shop", key::SHOP),
        ("/api/v1/magic-pool", key::MAGIC_POOL),
        ("/api/v1/pool", key::MAGIC_POOL),
        ("/api/v1/promo", key::PROMO_BUY),
        ("/api/v1/donate", key::MAGIC_POOL),
        ("/api/v1/fundings", key::MAGIC_POOL),
        ("/api/v1/resurrections", key::RESURRECTIONS),
        // 社区
        ("/api/v1/forums", key::FORUMS),
        ("/api/v1/messages", key::MESSAGES),
        ("/api/v1/staffmessages", key::MESSAGES),
        ("/api/v1/friends", key::FRIENDS),
        ("/api/v1/offers", key::OFFERS),
        ("/api/v1/requests", key::REQUESTS),
        ("/api/v1/subtitles", key::SUBTITLES),
        ("/api/v1/preserve", key::PRESERVE),
        ("/api/v1/social", key::SOCIAL), // 濒危/组队/赛季（默认关，走 social 键）
        ("/api/v1/textbooks", key::TEXTBOOKS),
        ("/api/v1/shoutbox", key::SHOUTBOX),
        // 运营
        ("/api/v1/attendance", key::ATTENDANCE),
        ("/api/v1/medals", key::MEDALS),
        ("/api/v1/dressup", key::DRESSUP),
        ("/api/v1/avatar-frames", key::DRESSUP),
        ("/api/v1/jixiao", key::JIXIAO),
        ("/api/v1/tasks", key::TASKS),
        ("/api/v1/push", key::PUSH),
    ];
    TABLE
        .iter()
        .find(|(p, _)| path == *p || path.starts_with(&format!("{}/", p)))
        .map(|(_, k)| *k)
}

/// 精确路径 → 模块键（不适合前缀表达的单点端点）。
fn exact_route_module(path: &str) -> Option<&'static str> {
    const TABLE: &[(&str, &str)] = &[
        ("/api/v1/me/vouchers", key::VOUCHERS),
        ("/api/v1/me/vouchers/use", key::VOUCHERS),
        ("/api/v1/wishlist", key::WISHLIST),
        ("/api/v1/wishlist/remove", key::WISHLIST),
        ("/api/v1/me/exams", key::EXAMS),
        ("/api/v1/me/hr", key::EXAMS), // 新人考核(HR)与 exams 同域口径
        ("/api/v1/me/hr/pardon", key::EXAMS),
    ];
    TABLE.iter().find(|(p, _)| path == *p).map(|(_, k)| *k)
}

/// 模块网关中间件：非 /api/v1 与管理端点直通；业务端点按映射表拦截（fail-close）。
/// 守卫失败直接以 DomainError 信封短路（4101 三语文案，task-local 由外层 locale_mw 注入）。
pub async fn module_gate_mw(
    req: actix_web::dev::ServiceRequest,
    next: actix_web::middleware::Next<impl actix_web::body::MessageBody>,
) -> actix_web::Result<
    actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>,
> {
    let path = req.path();
    let module_key = if let Some(m) = exact_route_module(path) {
        Some(m)
    } else {
        // 管理端点免检：站长必须始终能进设置中心去「打开」模块（§5.5）
        if path.starts_with("/api/v1/admin") || !path.starts_with("/api/v1") {
            None
        } else {
            route_module(path)
        }
    };
    if let Some(k) = module_key {
        let state = req
            .app_data::<web::Data<Arc<AppState>>>()
            .cloned()
            .expect("AppState registered");
        // 守卫失败 → 提前短路，不进业务 handler
        if let Err(e) = state.require_module(k).await {
            return Err(actix_web::Error::from(e));
        }
    }
    next.call(req).await
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

    /// 网关映射表抽验：关键前缀命中、核心/管理端点不受影响
    #[test]
    fn gateway_table_keys_valid() {
        for p in [
            "/api/v1/games",
            "/api/v1/farm",
            "/api/v1/gomoku",
            "/api/v1/contests",
            "/api/v1/bank",
            "/api/v1/shop",
            "/api/v1/magic-pool",
            "/api/v1/forums",
            "/api/v1/textbooks",
            "/api/v1/jixiao",
            "/api/v1/shoutbox",
        ] {
            assert!(route_module(p).is_some(), "gateway missing {p}");
        }
        // 核心层端点不映射任何模块
        assert!(route_module("/api/v1/torrents").is_none());
        assert!(route_module("/api/v1/me").is_none());
        assert!(exact_route_module("/api/v1/me/vouchers").is_some());
        assert!(exact_route_module("/api/v1/me/spark").is_none());
        // /bank/demand/deposit 这类更深路径也归 bank
        assert_eq!(
            route_module("/api/v1/bank/demand/deposit"),
            Some(key::BANK)
        );
    }
}
