//! 种子域仓储与查询（M02/M03）：游标分页 + 覆盖索引（§6.2）。
//! 按域拆分（300 行门禁）：行类型与过滤器在 types.rs，列表查询构造在
//! list.rs，详情/文件/感谢在 detail.rs，评论/点赞/收藏在 interact.rs，
//! 编辑/删除/恢复/抓取/NFO/求续种/标签在 manage.rs，下载计费与站点统计在
//! charge.rs；全部 pub use re-export，crate::torrents::xxx 路径不变。

mod charge;
mod detail;
mod interact;
mod list;
mod list_noclamp;
mod list_noclamp_as;
mod manage;
mod types;

pub use charge::*;
pub use detail::*;
pub use interact::*;
pub use list::*;
pub use list_noclamp::*;
pub use manage::*;
pub use types::*;
