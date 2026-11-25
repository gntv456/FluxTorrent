//! 对局战绩 SQL（GET /games/rounds）。自 `games_http/game_rounds.rs`
//! 下沉（分层宪法：H 层新文件零 SQL；拆件时把查询一起搬来 R 层）。
//!
//! 口径：以扣注流水为主体，按幂等键 LEFT JOIN 派彩。value 是物品档
//! 等值；历史幂等键兼容 `game-bs-win:` 与 `game-<ref_type>-win:`。

use crate::errors::{DomainError, DomainResult};

/// 战绩行（一局一条）。
#[derive(sqlx::FromRow, serde::Serialize)]
pub(crate) struct GameRoundRow {
    pub game: String,
    pub bet: i64,
    pub payout: i64,
    pub value: i64,
    pub net: i64,
    pub at: chrono::DateTime<chrono::Utc>,
}

/// 近 90 天战绩（按 id DESC）；`game` 过滤可选。
pub(crate) async fn game_rounds(
    db: &sqlx::PgPool,
    uid: i64,
    game: Option<&str>,
    limit: i64,
) -> DomainResult<Vec<GameRoundRow>> {
    let rows: Vec<GameRoundRow> = sqlx::query_as(
        "SELECT l.ref_type AS game, \
                l.amount AS bet, \
                COALESCE(w.amount, 0) AS payout, \
                COALESCE(d.value, COALESCE(w.amount, 0)) AS value, \
                l.amount + COALESCE(w.amount, 0) AS net, \
                l.created_at AS at \
         FROM spark_ledger l \
         LEFT JOIN arcade_draw_results d \
           ON d.idempotency_key = l.idempotency_key \
         LEFT JOIN spark_ledger w \
           ON w.idempotency_key IN ( \
                'game-' || l.ref_type || '-win:' || l.idempotency_key, \
                'game-bs-win:' || l.idempotency_key \
              ) \
         WHERE l.user_id = $1 AND l.kind = 'game' AND l.amount < 0 \
           AND l.ref_type IN ('scratch', 'bigsmall', 'jgg',
                              'capsule', 'fruit', 'wheel', 'fishing',
                              'mole', 'doudizhu') \
           AND ($2::text IS NULL OR l.ref_type = $2) \
           AND l.created_at > now() - interval '90 days' \
         ORDER BY l.id DESC LIMIT $3",
    )
    .bind(uid)
    .bind(game)
    .bind(limit)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(rows)
}
