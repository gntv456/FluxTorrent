//! 社交层（0102）：濒危预警雷达 / 组队契约 / 赛季 / 信誉。
//!
//! ## 与现有复活体系的关系（重要）
//!
//! FluxTorrent 已有完整的「死种复活」闭环（0073，U3D Graveyard 口径）：
//!   `ops_http.rs`  GET /resurrections · POST /resurrections/claim · GET /resurrections/mine
//!   条件：seeders = 0 且 30 天无 snatches 活动；不可领自己的种；奖励 5000 火花 + 免费券
//!
//! 本模块**不重复实现它**，只补一个它没有的环节：
//!
//! | | 现有 resurrections | 本模块 endangered |
//! |---|---|---|
//! | 对象 | 已死的资源（seeders = 0） | **濒危但有救的资源（seeders <= 阈值）** |
//! | 定位 | 死种复活（后置抢救） | **预警雷达（前置保种）** |
//! | 动作 | 认领并做种 required_hours | 只读展示 + 引导去保种 |
//!
//! 前置保种的边际成本远低于后置复活——资源一旦死到 0 做种者，能否救活取决于还有没有人
//! 留着完整数据。所以「还没死的时候提醒大家」比「死了再救」有价值得多，且实现成本极低。
//!
//! ## 复用而非重建
//!   做种激励   worker::jobs::seeding_reward
//!   保种结算   worker::jobs::preserve_settle / preserve_exit
//!   账本       spark_ledger（分区表，幂等键应用层先查后插）
//!   反作弊     cheat_events + users.status
//!   复活流程   resurrections（本模块只读它的状态用于展示「已有人在救」）
//!
//! 按域拆分（300 行门禁）：濒危雷达/开关助手在 endangered.rs，
//! 组队发起/加入在 team.rs，我的队伍/退出/招募列表在 team_members.rs。

mod endangered;
mod team;
mod team_members;

pub use endangered::endangered_list;
pub use team::{team_create, team_join};
pub use team_members::{team_leave, team_list, team_mine};
