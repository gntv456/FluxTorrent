//! 种子列表免钳制入口（M02 管理面）：noclamp 直查（上限放宽）。
//! 从 torrents/list.rs 按域拆出。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

use super::list_noclamp_as::list_torrents_noclamp_as;
use super::types::{ListCursor, TorrentFilter, TorrentPage};

/// 「可发货」谓词（P-433）：订阅类出口只能广告有原始存档的种。下载口对
/// 无存档种回 400「种子文件缺失…请联系管理组补档」，而 Prowlarr/Sonarr/
/// qBittorrent 会按 guid 自动重试同一枚，用户侧只看到永久失败的红色条目。
/// RSS 的 SQL 与 Torznab 的回收共用这一份，别在别处另写一句。
pub const HAS_ARCHIVE_CLAUSE: &str =
    "EXISTS (SELECT 1 FROM torrent_files tf WHERE tf.torrent_id = t.id)";

/// 不做 50 上限钳制的列表查询：仅供 Torznab 等需要 offset+limit>50 深翻页的外部端点，
/// 调用方必须自行 clamp 防滥用（见 economy_http torznab_search）。viewer=0（无状态筛选视角）。
pub async fn list_torrents_noclamp(
    db: &PgPool,
    filter: &TorrentFilter,
    cursor: Option<ListCursor>,
    limit: i64,
) -> DomainResult<TorrentPage> {
    list_torrents_noclamp_as(db, filter, cursor, limit, 0).await
}

/// 凭证类订阅出口专用（P-324）：在 noclamp 之上回收「付费且未购」的种，
/// 以及「无原始存档、下载口必 400」的种（P-433）。
///
/// 为什么必须回收：这类出口的 item 直接带 `download.php?id&passkey`（本人
/// passkey），Prowlarr/Sonarr/Radarr 会**自动**抓 enclosure，而
/// `charge_for_download` 的口径是「首次下载即扣魔力、写已购流水」——把付费种
/// 列出来等于替用户花钱，且客户端侧没有任何确认环节。RSS 在深测 2026-10-03
/// （`rss_http.rs` 的 `exclude_paid`）就定了同一条口径，Torznab 当时漏接。
///
/// 放行三类，与 RSS 谓词逐条对齐：`price <= 0`（免费）、`owner_id = viewer`
/// （发布者本人下载不扣费）、已购（`torrent_purchases`）。
/// `hide_paid = false` 等价 RSS 的 `?paid=0`：显式要全量监控的订阅器逃生口。
///
/// `total_estimate` 保持未回收的原值——它是整结果集的估算数，减去「本页回收
/// 掉几条」只会得到一个两头都不对的数字；Torznab feed 不输出该字段。
pub async fn list_torrents_noclamp_for_sub(
    db: &PgPool,
    filter: &TorrentFilter,
    cursor: Option<ListCursor>,
    limit: i64,
    viewer: i64,
    hide_paid: bool,
) -> DomainResult<TorrentPage> {
    let mut page =
        list_torrents_noclamp_as(db, filter, cursor, limit, viewer).await?;
    if page.items.is_empty() {
        return Ok(page);
    }
    let ids: Vec<i64> = page.items.iter().map(|t| t.id).collect();
    // P-433：无存档种的回收与 hide_paid 无关（逃生口也只放付费，不放
    // 「点下去必 400」的种），两条条件合成一次查询，按 id 集合回收。
    let sql = format!(
        "SELECT t.id FROM torrents t WHERE t.id = ANY($1) AND ((NOT {}) \
         OR ($2::bool AND t.price > 0 AND t.owner_id IS DISTINCT FROM $3 \
         AND NOT EXISTS (SELECT 1 FROM torrent_purchases tp \
         WHERE tp.user_id = $3 AND tp.torrent_id = t.id)))",
        HAS_ARCHIVE_CLAUSE
    );
    let blocked: Vec<i64> = sqlx::query_scalar(&sql)
        .bind(&ids)
        .bind(hide_paid)
        .bind(viewer)
        .fetch_all(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if !blocked.is_empty() {
        let set: std::collections::HashSet<i64> = blocked.into_iter().collect();
        page.items.retain(|t| !set.contains(&t.id));
    }
    Ok(page)
}
