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
mod upload_files_promo;
mod upload_sections;

pub use collections::*;
pub use download::*;
pub use group::*;
pub use ptgen::*;
pub use upload::*;
