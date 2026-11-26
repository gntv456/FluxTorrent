//! 斗地主对局快照持久化（从 doudizhu_play_repo.rs 拆出：门禁）。

use super::doudizhu_game::{GameState, Phase};
use super::doudizhu_repo::GameRow;
use crate::errors::{DomainError, DomainResult};

fn dberr(e: sqlx::Error) -> DomainError {
    DomainError::Internal(anyhow::Error::from(e))
}

/// 持久化快照（未终局）。
pub(crate) async fn persist_state_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    row: &mut GameRow,
) -> DomainResult<()> {
    let snapshot = serde_json::to_value(&row.state)
        .map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;
    let status = phase_status(&row.state);
    sqlx::query(
        "UPDATE arcade_doudizhu_games \
            SET state = $2, status = $3, updated_at = now() \
          WHERE id = $1",
    )
    .bind(row.id)
    .bind(&snapshot)
    .bind(status)
    .execute(&mut **tx)
    .await
    .map_err(dberr)?;
    Ok(())
}

pub(crate) fn phase_status(state: &GameState) -> &'static str {
    match state.phase {
        Phase::Bidding => "bidding",
        Phase::Grab | Phase::Discard | Phase::Double | Phase::Playing => {
            "playing"
        }
        Phase::Done => "settled",
    }
}
