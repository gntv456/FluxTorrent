//! 甜梦钓场（2026-11-21）：站内完整渔场。
//!
//! 与既有「竞猜式钓鱼」（games_http/fishing.rs）并存但独立：
//! 那套是一注一抽的魔力池，这套是养成 + 溜鱼手感的完整渔场。
//!
//! **经济口径（2026-11-24）**：钓场是魔力的**回收口**，不发行任何货币 ——
//! `magic` 就是站点魔力的镜像。玩家花魔力的地方：补体力 40 / 次、装备、
//! 场景、背包扩容、自动垂钓；**任何路径都不返魔力**（渔获 / 订单 / 全清奖 /
//! 成就 / 签到一律只发免费竿或只记估值）。资金闸在 http.rs 的 `mirror_magic`
//! （只扣不加），规则层回归见 pond_tests.rs，经济口径锁定见
//! pond_sink_tests.rs 的 `no_path_mints_*`。
//!
//! 服务端做三件事：保存存档、掷骰发鱼、校验动作，客户端负责表现与溜鱼手感。

mod actions;
mod bait;
mod data;
mod http;
#[cfg(test)]
mod pond_bait_tests;
#[cfg(test)]
mod pond_sink_tests;
#[cfg(test)]
mod pond_tests;
mod rules;
mod sign;
mod state;
mod state_ops;

pub(super) use http::*;
