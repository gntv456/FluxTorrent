//! 种子域仓储（torrent_download + torrent_detail 的归组，2026-11-24）。
//!
//! 2026-10-15 `.torrent` 构建链的 SQL 从 `publish_http/download.rs` 下沉为
//! `repo/torrent_download.rs`；2026-11-24 详情聚合个人段的 SQL 从
//! `torrent_http/aggregate` 下沉（FRAME §2「H 不 import sqlx」，分层门禁对
//! 新 H 层文件带 SQL 零容忍）。两者同属种子域，且 `repo/mod.rs` 的模块
//! 登记表已到 300 行上限，故按 `repo/community` 的先例建目录子模块归组，
//! 登记表 `torrent_download` 一行换成 `torrent` 一行，不再增长。
//!
//! - `download`：`.torrent` 构建链取数
//! - `detail`：详情聚合个人段（viewer 态布尔 / 保种认领位）
//!
//! 四个下载函数在本层 re-export：调用方写 `repo::torrent::torrent_name`
//! 即可，不必知道它住在哪个子文件。

pub mod detail;
pub mod download;

pub use download::{
    extra_trackers, raw_torrent_file, torrent_exists, torrent_name,
};
