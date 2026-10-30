//! 种子域仓储与查询（M02/M03）：游标分页 + 覆盖索引（§6.2）。
//! 按域拆分（300 行门禁）：行类型与过滤器在 types.rs，列表查询构造在
//! list.rs，详情/文件/感谢在 detail.rs，评论/点赞/收藏在 interact.rs，
//! 编辑/删除/恢复/抓取/NFO/求续种/标签在 manage.rs，删除表单取数与站方处置
//! 在 delete_dispose.rs，下载计费与站点统计在
//! charge.rs；全部 pub use re-export，crate::torrents::xxx 路径不变。

mod cat_map;
mod charge;
mod comments;
mod count_bind;
mod cursor;
// 删除表单取数与站方处置（火花回收/禁发种/台账）
pub(crate) mod delete_dispose;
mod detail;
mod interact;
mod list;
mod list_noclamp;
mod list_noclamp_as;
// 抓轨日志读路径（0312）
mod logs;
mod manage;
pub(crate) mod manage_perm;
pub(crate) mod promo;
pub(crate) mod related;
mod reseed;
pub(crate) mod section_filter;
mod section_pred;
mod syn_search;
mod tag_pred;
mod types;
mod viewer_preds;

pub use cat_map::{newznab_name, CatMap, MediaMap, NEWZNAB_OTHER};
pub use charge::*;
// LIKE 通配转义（批九 RSS 出口对齐主列表口径）
pub use comments::*;
pub use delete_dispose::*;
pub use detail::*;
pub use interact::*;
pub(crate) use list::esc_like;
// 作品聚合筛选复用列表侧同一套谓词（works_filter.rs）
pub(crate) use list::{extra_preds, ExtraSlots};
pub use reseed::*;
pub(crate) use syn_search::search_pred_with_syn;
pub(crate) use tag_pred::tag_pred;
pub(crate) use viewer_preds::{bookmark_pred, status_pred};
pub(crate) mod view_scope;
mod visibility;
pub use list::*;
pub use list_noclamp::*;
pub use logs::*;
pub use manage::*;
pub use related::*;
pub use section_filter::SectionFilter;
/// 多维筛选谓词（B3）：后台管理列表复用前台同一实现。
pub(crate) use section_pred::section_where;
pub(crate) use section_pred::sync_legacy_columns;
pub use types::*;
pub use visibility::*;
