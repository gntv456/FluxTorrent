//! G31-A 只读公示面（方案《抽卡玩法落地方案-2026-09-27》§5）：稀有度 token、
//! 卡定义、卡池概率（**表定与含保底综合两列**，方案红线：只公示表定会放行
//! 实际倒灌经济的池子——样张实测同一池表定 91.6% / 综合 105.8%）。
//!
//! 数学单源 `crate::gacha_math`（与 `packages/gacha-math` 共享
//! `gacha_math_vectors.json`，逐位镜像）；公示输出直接来自该层，不另算一遍。
//!
//! 匿名口径（方案 §5 要求当场定死）：公示信息不含用户数据 → **匿名可读**；
//! 卡面由前端按 rarities token 程序化渲染（样张实证 100% 程序化 SVG 可达），
//! 不取 attachments——不给实体图开匿名路径，公示面也不依赖它。
//! 模块关闭时 gateway 前缀映射（/api/v1/gacha → key::GACHA）整段 404，
//! 路由随开关消失（方案 §5 断言「路由消失」的机制位）。

mod craft;
mod draw;
mod pool;
mod rates;
mod roll;
mod staff;

use actix_web::Scope;

pub fn mount_gacha(scope: Scope) -> Scope {
    scope
        .service(rates::gacha_rarities)
        .service(rates::gacha_cards)
        .service(rates::gacha_banners)
        .service(rates::gacha_banner_rates)
        .service(draw::gacha_draw)
        .service(craft::gacha_dismantle)
        .service(craft::gacha_exchange)
        .service(craft::gacha_levelup)
        // staff 发放挂 /admin 前缀（gateway 对 /api/v1/admin 免检——站长必须
        // 始终能进设置中心；权限由 authz::require_perm 把守）
        .service(staff::gacha_grant_tickets)
        .service(staff::gacha_grant_shards)
}
