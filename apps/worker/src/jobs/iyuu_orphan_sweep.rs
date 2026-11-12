//! IYUU 辅种缓存孤儿清理：删掉种子已不存在的 `iyuu_crossseed` 行。
//!
//! 全仓对这张表没有 DELETE——读侧（详情页面板）和写侧（选种）都从
//! torrents 出发 JOIN，孤儿行永远不会被读到，纯粹是死重量。删种/合并/
//! 重建后行无限累积，日频扫一次兜底。
//!
//! NOT IN 在 PG 16 优化器下走 anti-hash-join，行数只有万级，秒完。

use anyhow::Result;
use sqlx::PgPool;

/// 一轮清理。返回删除的行数。
pub(crate) async fn iyuu_orphan_sweep(db: &PgPool) -> Result<u64> {
    let n = sqlx::query(
        "DELETE FROM iyuu_crossseed \
         WHERE torrent_id NOT IN (SELECT id FROM torrents)",
    )
    .execute(db)
    .await?
    .rows_affected();
    if n > 0 {
        tracing::info!(n, "iyuu crossseed orphan rows swept");
    }
    Ok(n)
}
