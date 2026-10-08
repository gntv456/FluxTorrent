//! 发布 / 下载（M04 / M05）+ 聚合组（0069）+ PTGen 元数据抓取。
//! 按域拆分（300 行门禁）：NFO/文本助手与 PTGen 在 ptgen.rs，发种 upload 在
//! upload.rs，聚合组在 group.rs，.torrent 生成下载在 download.rs。
//! 路由注册沿用 http.rs 的 crate::publish_http::fn 引用（glob re-export）。

mod collections;
mod descr_image;
mod download;
mod group;
mod ptgen;
mod upload;
// 发种前置校验（0288）：把「报错时种子已入库」的校验全部提到 INSERT 之前
mod upload_body;
mod upload_fields;
// 抓轨日志闸门与落库（0312）
mod upload_logcheck;
mod upload_precheck;
// 墓碑复活（0288）：同作者删除后重发同一 .torrent 走 UPDATE 复用原 id
mod upload_files_promo;
mod upload_revive;
mod upload_suggest;
// 多维属性解析/写入（B2 六类型）：编辑口与批量口也要调，故公开
pub(crate) mod upload_sections;

pub use collections::*;
pub use download::*;
pub use group::*;
pub use ptgen::*;
pub use upload::*;
