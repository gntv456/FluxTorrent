//! 作品元数据回填候选查询（2026-11-02 豆瓣锚点批）：publish_http 的
//! backfill 端点触达 SQL 时下沉于此（分层宪法①：H 层零 sqlx）。

use crate::errors::{DomainError, DomainResult};

/// 有任一外部锚点（imdb_id/imdb_id·meta/tmdb_id/douban_id）的种子，
/// 按作品键去重每键取一个代表 id。
pub async fn work_backfill_candidates(
    db: &sqlx::PgPool,
) -> DomainResult<Vec<i64>> {
    let wk = crate::torrent_http::works_sql::wk_expr();
    let ids: Vec<(i64,)> = sqlx::query_as(&format!(
        "SELECT DISTINCT ON ({wk}) t.id FROM torrents t \
         WHERE COALESCE(NULLIF(t.imdb_id,''), '') <> '' \
            OR COALESCE(t.media_info->>'imdb_id', '') <> '' \
            OR COALESCE(t.media_info->>'tmdb_id', '') <> '' \
            OR COALESCE(t.media_info->>'douban_id', '') <> '' \
         ORDER BY {wk}, t.id"
    ))
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ids.into_iter().map(|(i,)| i).collect())
}
