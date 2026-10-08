//! 种子浏览与互动（M02/M03/M07/M10）：列表/详情/文件/评论/感谢/收藏/
//! 编辑/定价/恢复/重提/删除/抓取列表/NFO/求续种/标签。
//! 从 http.rs 机械外移（审查路线图第 4 周「拆上帝文件」第六段）。

//! 按域拆分（300 行门禁）：列表与只读详情在 list.rs，评论区写操作与
//! 感谢/收藏/求续种在 interact.rs，编辑/定价/恢复/删除/标签管理在 manage.rs。

mod aggregate;
mod batch;
mod comments_like;
mod detail;
mod interact;
mod list;
mod magnet;
mod manage;
// 抓轨日志正文（0312）
mod logs;
mod peers;
mod query;
mod related;
mod sec_params;
mod suggest;

pub use aggregate::*;
pub use batch::*;
pub use detail::*;
// 查询参数归一器（0267）：对外兼容层要用同一套「体积/优惠/日期/标签」解析口径，
// 复制一份必然漂移（历史上 RSS/Torznab 的促销标签就是这么各写一份的）
pub use interact::*;
pub use list::*;
pub use logs::*;
pub use magnet::*;
pub use manage::*;
pub use peers::*;
pub(crate) use query::{
    norm_date, norm_promo, norm_tags, norm_text, parse_size,
};
pub use related::*;
/// 多维筛选参数解析：后台管理列表也要用同一实现（B3）。
pub(crate) use sec_params::parse_section_params;
pub use suggest::*;
