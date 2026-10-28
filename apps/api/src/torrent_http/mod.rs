//! 种子浏览与互动（M02/M03/M07/M10）：列表/详情/文件/评论/感谢/收藏/
//! 编辑/定价/恢复/重提/删除/抓取列表/NFO/求续种/标签。
//! 从 http.rs 机械外移（审查路线图第 4 周「拆上帝文件」第六段）。

//! 按域拆分（300 行门禁）：列表与只读详情在 list.rs，评论区写操作与
//! 感谢/收藏/求续种在 interact.rs，编辑/定价/恢复/标签管理在 manage.rs，
//! 删除与附带处置在 torrent_delete.rs。

mod aggregate;
mod batch;
mod comments_like;
mod detail;
mod interact;
mod interact_tags;
mod list;
mod notify;
mod thanks_spark;
mod works_watch;
// 列表入口的偏好折算与区间对调（P-275）
mod list_prefs;
mod magnet;
mod manage;
// 抓轨日志正文（0312）
mod logs;
mod peers;
mod query;
mod related;
// CUE 合规判定（G3 后半）
mod cue;
// 文件树冲突检测（G13）
mod file_conflict;
// 预览片段（0337 试读/试听）
mod preview;
mod sec_params;
mod suggest;
// 删除与站方处置（理由/火花回收/禁发种/台账）
mod torrent_delete;
// 举报/替换（0336 trumping；裁决侧面在 trump_admin）
mod trump;
mod trump_admin;
// 作品聚合（资源页「作品聚合」视图 + 作品详情）：同片多版本归一部作品
mod works;
mod works_rows_sql;
pub(crate) mod works_sql;

pub use aggregate::*;
pub use batch::*;
pub use cue::*;
pub use detail::*;
pub use file_conflict::*;
// 查询参数归一器（0267）：对外兼容层要用同一套「体积/优惠/日期/标签」解析口径，
// 复制一份必然漂移（历史上 RSS/Torznab 的促销标签就是这么各写一份的）
pub use interact::*;
pub use list::*;
pub use logs::*;
pub use magnet::*;
pub use manage::*;
pub use peers::*;
pub use preview::*;
pub(crate) use query::{
    anonymous_or_err, date_or_err, norm_date, norm_promo, norm_tags, norm_text,
    parse_size, promo_or_err, size_or_err,
};
pub use related::*;
/// 多维筛选参数解析：后台管理列表也要用同一实现（B3）。
pub(crate) use sec_params::parse_section_params;
pub use suggest::*;
pub use torrent_delete::*;
pub use trump::*;
pub use trump_admin::*;
pub use works::*;
pub(crate) use works_watch::work_watch;
// ===== F4 utoipa 聚合出口（种子域 38 条路由的 #[utoipa::path] 入口）====
// openapi_http 的 OpenApi 聚合要从 crate 外部引用这些 handler，
// 而子模块是私有 `mod`。这里做一层显式重导出，而不是把 20 个子模块
// 全改成 pub —— 后者会把内部结构一并暴露出去。
pub(crate) use aggregate::torrent_aggregate;
pub(crate) use batch::batch_download;
pub(crate) use cue::cue_check;
pub(crate) use detail::comments;
pub(crate) use detail::torrent_detail_ext;
pub(crate) use detail::torrent_files;
pub(crate) use detail::torrent_nfo;
pub(crate) use detail::torrent_snatches;
pub(crate) use detail::torrent_thanks;
pub(crate) use file_conflict::file_conflicts;
pub(crate) use interact::create_comment;
pub(crate) use interact::delete_bookmark;
pub(crate) use interact::delete_comment;
pub(crate) use interact::do_bookmark;
pub(crate) use interact::do_thank;
pub(crate) use interact::request_reseed;
pub(crate) use interact::toggle_comment_like;
pub(crate) use interact_tags::torrent_tag_put;
pub(crate) use interact_tags::torrent_tags;
// utoipa 的 paths! 在「handler 可见的模块」里找 __path_* 类型，
// 拆文件后 handler 的 __path_* 生在新模块里——得在这里转出。
pub(crate) use interact_tags::{__path_torrent_tag_put, __path_torrent_tags};
pub(crate) use logs::log_adjust;
pub(crate) use logs::torrent_log_body;
pub(crate) use magnet::torrent_magnet;
pub(crate) use manage::edit_torrent;
pub(crate) use manage::restore_torrent;
pub(crate) use manage::resubmit_torrent;
pub(crate) use manage::set_torrent_price;
pub(crate) use peers::torrent_peers;
pub(crate) use preview::preview_add;
pub(crate) use preview::preview_list;
pub(crate) use preview::preview_remove;
pub(crate) use related::torrent_related;
pub(crate) use suggest::torrent_suggest;
pub(crate) use trump::torrent_trump;
pub(crate) use trump::torrent_trumps;
pub(crate) use trump_admin::trump_list;
pub(crate) use trump_admin::trump_resolve;
