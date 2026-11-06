//! 种子做种/下载计数的点刷（announce / sweep / 对账三处共用）。
//!
//! 口径：`torrents.seeders / leechers / times_completed` = snatches 侧的
//! 聚合计数（权威在流水，快照仅展示）。三处点刷曾各写一份同款 SQL，
//! 同一个坑踩了三次，逐条钉死：
//!
//! ① **归零不写回**：旧写法 `UPDATE torrents … FROM (聚合) WHERE
//!    c.torrent_id = t.id` 只命中聚合里**有行**的种子 ⇒ 最后一个做种者
//!    断开后计数卡在 1 不落 0（用户报的「做种数不对」同源）。现用
//!    LEFT JOIN + COALESCE，零值也写回。
//! ② **Rust 续行吃空格**：字符串里的 `\` + 换行会吃掉下一行的前导空白，
//!    于是 `UPDATE torrents t\` + `SET seeders …` 拼成 `tSET seeders`
//!    ——`tSET` 被当成别名，报 "syntax error at or near seeders"。
//!    ⇒ 本文件**一律用 raw string**（r#"…"#），不用 `\` 续行拼 SQL。
//! ③ **一次刷太多**：聚合走 0464/0466 的部分索引（index-only scan），但
//!    snatches 的可见性图一旦被大批 UPDATE 弄脏就退化成回表——实测
//!    5,000 个种子的点刷读了 3.1 万个磁盘块、耗时 104 秒，把 announce
//!    的消费 tick 整轮堵死。⇒ 分批（每批 1,000 个种子），单批的 IO 与
//!    锁持有时间都压到秒级。

use sqlx::postgres::PgPool;
use sqlx::{Executor, Postgres};

/// 单批点刷的种子数。
const FLUSH_CHUNK: usize = 1_000;

/// 点刷给定种子集合的三个计数，返回被改写的种子数。
pub(crate) async fn refresh_peer_counts(
    db: &PgPool,
    ids: &[i64],
) -> anyhow::Result<u64> {
    refresh(db, ids, true).await
}

/// 热路径点刷：只重算 seeders/leechers。
///
/// `times_completed` 取决于 `completed_at`，只有「下载完成」时才变（远比
/// announce 稀疏），热路径带上它等于每次都扫一遍 completed 索引（实测
/// 占点刷成本的 2/3）。该口径由对账任务（全量、含 times_completed）兜底。
pub(crate) async fn refresh_peer_counts_light(
    db: &PgPool,
    ids: &[i64],
) -> anyhow::Result<u64> {
    refresh(db, ids, false).await
}

async fn refresh(
    db: &PgPool,
    ids: &[i64],
    with_completed: bool,
) -> anyhow::Result<u64> {
    let mut changed = 0u64;
    for chunk in ids.chunks(FLUSH_CHUNK) {
        changed += flush_chunk(db, chunk, with_completed).await?;
    }
    Ok(changed)
}

/// 事务内点刷（sweep 用：与僵尸清理同事务 + lock_timeout）。
pub(crate) async fn refresh_peer_counts_in_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    ids: &[i64],
) -> anyhow::Result<u64> {
    let mut changed = 0u64;
    for chunk in ids.chunks(FLUSH_CHUNK) {
        changed += flush_chunk(&mut **tx, chunk, true).await?;
    }
    Ok(changed)
}

/// 全量点刷（对账用）：按 id 分页扫 torrents，逐批重算。
pub(crate) async fn refresh_all_peer_counts(
    db: &PgPool,
) -> anyhow::Result<u64> {
    let mut changed = 0u64;
    let mut last_id = 0i64;
    loop {
        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM torrents WHERE id > $1 ORDER BY id LIMIT $2",
        )
        .bind(last_id)
        .bind(FLUSH_CHUNK as i64)
        .fetch_all(db)
        .await?;
        if ids.is_empty() {
            break;
        }
        last_id = *ids.last().unwrap_or(&last_id);
        changed += flush_chunk(db, &ids, true).await?;
    }
    Ok(changed)
}

/// 三计数全刷（sweep / 对账用）。
const FLUSH_SQL_FULL: &str = r#"
        WITH sd AS (
            SELECT torrent_id, count(*) AS n FROM snatches
             WHERE seeding AND torrent_id = ANY($1) GROUP BY torrent_id
        ), lc AS (
            SELECT torrent_id, count(*) AS n FROM snatches
             WHERE leeching AND torrent_id = ANY($1) GROUP BY torrent_id
        ), tc AS (
            SELECT torrent_id, count(*) AS n FROM snatches
             WHERE completed_at IS NOT NULL AND torrent_id = ANY($1)
             GROUP BY torrent_id
        )
        UPDATE torrents t
           SET seeders = COALESCE(sd.n, 0),
               leechers = COALESCE(lc.n, 0),
               times_completed = COALESCE(tc.n, 0)
          FROM unnest($1::bigint[]) AS t2(id)
          LEFT JOIN sd ON sd.torrent_id = t2.id
          LEFT JOIN lc ON lc.torrent_id = t2.id
          LEFT JOIN tc ON tc.torrent_id = t2.id
         WHERE t.id = t2.id
           AND (t.seeders, t.leechers, t.times_completed) IS DISTINCT FROM
               (COALESCE(sd.n, 0), COALESCE(lc.n, 0), COALESCE(tc.n, 0))
        "#;

/// 热路径点刷：只算 seeders/leechers（times_completed 由对账兜底）。
const FLUSH_SQL_LIGHT: &str = r#"
        WITH sd AS (
            SELECT torrent_id, count(*) AS n FROM snatches
             WHERE seeding AND torrent_id = ANY($1) GROUP BY torrent_id
        ), lc AS (
            SELECT torrent_id, count(*) AS n FROM snatches
             WHERE leeching AND torrent_id = ANY($1) GROUP BY torrent_id
        )
        UPDATE torrents t
           SET seeders = COALESCE(sd.n, 0),
               leechers = COALESCE(lc.n, 0)
          FROM unnest($1::bigint[]) AS t2(id)
          LEFT JOIN sd ON sd.torrent_id = t2.id
          LEFT JOIN lc ON lc.torrent_id = t2.id
         WHERE t.id = t2.id
           AND (t.seeders, t.leechers) IS DISTINCT FROM
               (COALESCE(sd.n, 0), COALESCE(lc.n, 0))
        "#;

/// 单批点刷（raw string：换行与缩进原样保留，不受续行规则影响）。
async fn flush_chunk<'e, E>(
    executor: E,
    ids: &[i64],
    with_completed: bool,
) -> anyhow::Result<u64>
where
    E: Executor<'e, Database = Postgres>,
{
    let sql = if with_completed {
        FLUSH_SQL_FULL
    } else {
        FLUSH_SQL_LIGHT
    };
    let res = sqlx::query(sql).bind(ids).execute(executor).await?;
    Ok(res.rows_affected())
}
