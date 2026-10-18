//! 社区域仓储（community_http 的 SQL 唯一居所）。
//!
//! 2026-10-16 建立：社区域做 OpenAPI 注解化时按 300 行门禁拆出的 5 个
//! 新 H 文件带着存量 SQL，按「新 H 层文件零容忍」处理——把 SQL 下沉到
//! 仓储层，H 层只留鉴权、口径判断与 JSON 组装。
//!
//! 分文件是因为单文件装不下四个域的 SQL；每个文件对应一组路由。

pub mod ban_log;
pub mod boards;
pub mod follows;
pub mod forum_mine;
pub mod forum_side;
pub mod friends;
pub mod matches;
pub mod topic_delete;
pub mod user_directory;
