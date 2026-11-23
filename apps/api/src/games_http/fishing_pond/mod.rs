//! 甜梦钓场（2026-11-21）：站内完整渔场。
//!
//! 与既有「竞猜式钓鱼」（games_http/fishing.rs）并存但独立：
//! 那套是一注一抽的魔力池，这套是养成 + 溜鱼手感的完整渔场，货币是场内的
//! `magic`（不进站点魔力账）。服务端只做三件事：保存存档、掷骰发鱼、校验动作，
//! 客户端负责表现与张力操作。

mod data;
mod http;
mod rules;
mod state;

pub(super) use http::*;
