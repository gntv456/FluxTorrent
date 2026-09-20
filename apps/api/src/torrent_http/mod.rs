//! 种子浏览与互动（M02/M03/M07/M10）：列表/详情/文件/评论/感谢/收藏/
//! 编辑/定价/恢复/重提/删除/抓取列表/NFO/求续种/标签。
//! 从 http.rs 机械外移（审查路线图第 4 周「拆上帝文件」第六段）。

//! 按域拆分（300 行门禁）：列表与只读详情在 list.rs，评论区写操作与
//! 感谢/收藏/求续种在 interact.rs，编辑/定价/恢复/删除/标签管理在 manage.rs。

mod detail;
mod interact;
mod list;
mod manage;
mod query;

pub use detail::*;
pub use interact::*;
pub use list::*;
pub use manage::*;
