//! 种子列表免钳制入口（M02 管理面）：noclamp 直查（上限放宽）。
//! 从 torrents/list.rs 按域拆出。

use sqlx::PgPool;

use crate::errors::DomainResult;

use super::list_noclamp_as::list_torrents_noclamp_as;
use super::types::{TorrentFilter, TorrentPage};

/// 不做 50 上限钳制的列表查询：仅供 Torznab 等需要 offset+limit>50 深翻页的外部端点，
/// 调用方必须自行 clamp 防滥用（见 economy_http torznab_search）。viewer=0（无状态筛选视角）。
pub async fn list_torrents_noclamp(
    db: &PgPool,
    filter: &TorrentFilter,
    cursor: Option<i64>,
    limit: i64,
) -> DomainResult<TorrentPage> {
    list_torrents_noclamp_as(db, filter, cursor, limit, 0).await
}
