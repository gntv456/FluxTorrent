//! API 模块网关（§5.2 的收敛实现：路径前缀 → 模块映射）。
//! 从 modules.rs 机械外移。

use std::sync::Arc;

use actix_web::web;

use super::key;
use crate::state::AppState;

/// 路径前缀 → 模块键。命中即按该模块开关拦截。
/// 维护纪律 D2：新模块端点必须登记在此（契约测试无法枚举路由，靠评审清单把关）。
fn route_module(path: &str) -> Option<&'static str> {
    // 较长前缀先匹配（/bank/demand 也归 bank）
    const TABLE: &[(&str, &str)] = &[
        // 娱乐
        ("/api/v1/games", key::GAMES),
        ("/api/v1/gacha", key::GACHA),
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
        // 邀请系统（四审 L5 P1：此前 invites 从未进注册表，4 个端点不在网关、
        // 页面无守卫、入口硬编码，关掉别的模块也拿它没办法）
        ("/api/v1/invites", key::INVITES),
        ("/api/v1/messages", key::MESSAGES),
        ("/api/v1/staffmessages", key::MESSAGES),
        ("/api/v1/contactstaff", key::MESSAGES), // 给管理组发信（messages 域）
        ("/api/v1/stafftickets", key::MESSAGES), // 工单（messages 域）
        ("/api/v1/staff/leaks", key::MESSAGES), // 泄密 bot（messages 域展示面）
        ("/api/v1/friends", key::FRIENDS),
        ("/api/v1/offers", key::OFFERS),
        ("/api/v1/requests", key::REQUESTS),
        ("/api/v1/subtitles", key::SUBTITLES),
        ("/api/v1/preserve", key::PRESERVE),
        ("/api/v1/social", key::SOCIAL), // 濒危/组队/赛季（默认关，走 social 键）
        ("/api/v1/follows", key::FORUMS), // 论坛关注（0155）：与 /forums/feed 同域
        ("/api/v1/textbooks", key::TEXTBOOKS),
        ("/api/v1/shoutbox", key::SHOUTBOX),
        // 运营
        ("/api/v1/attendance", key::ATTENDANCE),
        ("/api/v1/medals", key::MEDALS),
        ("/api/v1/medal-wall", key::MEDALS), // 勋章墙（medals 域）
        ("/api/v1/dressup", key::DRESSUP),
        ("/api/v1/avatar-frames", key::DRESSUP),
        ("/api/v1/jixiao", key::JIXIAO),
        ("/api/v1/tasks", key::TASKS),
        ("/api/v1/push", key::PUSH),
        ("/api/v1/sticky-promos", key::PROMO_BUY), // 置顶促销展示（promo_buy 域）
    ];
    TABLE
        .iter()
        .find(|(p, _)| path == *p || path.starts_with(&format!("{}/", p)))
        .map(|(_, k)| *k)
        .or_else(|| exact_route_module(path))
}

/// 精确路径 → 模块键（不适合前缀表达的单点端点）。
fn exact_route_module(path: &str) -> Option<&'static str> {
    const TABLE: &[(&str, &str)] = &[
        ("/api/v1/me/vouchers", key::VOUCHERS),
        ("/api/v1/me/vouchers/use", key::VOUCHERS),
        ("/api/v1/wishlist", key::WISHLIST),
        ("/api/v1/wishlist/remove", key::WISHLIST),
        ("/api/v1/me/exams", key::EXAMS),
        // H&R 用户记录不再挂 exams：general 站型 exams 缺省关，而 hr_enforce
        // 执法独立于 exams 在跑——曾造成「执法在跑、用户看不到自己 H&R」的
        // 断链（2026-10-02 资深用户深测 P1-4）。H&R 无独立模块键，恒开放；
        // 站点要整体关 H&R 用 hr 相关 site_settings（hr_enforce 读的那组）。
        // 二审 G8 补登：/me 下的模块动作端点（网关无 /me 前缀规则，逐条登记）
        ("/api/v1/me/avatar-frame", key::DRESSUP), // 佩戴挂件（真实扣魔力）
        ("/api/v1/me/medals", key::MEDALS),
        ("/api/v1/me/notice-prefs", key::MEDALS),
        ("/api/v1/me/staffmessages", key::MESSAGES), // 三审 C-1：用户端管理组信件
        ("/api/v1/me/staffmessages/confirm", key::MESSAGES),
        ("/api/v1/pool/honor", key::MAGIC_POOL),
        ("/api/v1/medal-rarities", key::MEDALS), // 三审 C-1：稀有度字典
        // 四审 S2：/open/me/messages 与 /api/v1/messages 同数据域——此前它挂在
        // /api/v1 scope 内却不经映射表，关掉 messages 模块后站内信仍能从
        // OpenAPI Token 出口读到（两套鉴权、同一份隐私数据）。
        ("/api/v1/open/me/messages", key::MESSAGES),
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
            // 二审 G8 补登端点抽验
            "/api/v1/follows/mine",
            "/api/v1/contactstaff",
            "/api/v1/stafftickets",
            "/api/v1/medal-wall",
            "/api/v1/sticky-promos",
            "/api/v1/staff/leaks",
        ] {
            assert!(route_module(p).is_some(), "gateway missing {p}");
        }
        for p in [
            "/api/v1/open/me/messages",
            "/api/v1/me/avatar-frame",
            "/api/v1/me/medals",
            "/api/v1/me/notice-prefs",
            "/api/v1/pool/honor",
        ] {
            assert!(exact_route_module(p).is_some(), "gateway missing {p}");
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
