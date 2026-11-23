//! 猜大小「倒计时房间」仓储与懒结算（事务内）。
//!
//! 状态机：全站同时只有一个 `betting` 期（0515 部分唯一索引兜底）。每次进入房间
//! 先 `advance_open`：最新期未到期就原样返回；已到期则在同一事务内掷骰、按期内
//! 下注统一派彩、回填结算结果，再开出下一期。不依赖 worker 定时编排——房间没有
//! 玩家时不必空跑开奖；派彩仍走统一 `spark_ledger` 管线，幂等键防重复入账。

use chrono::{DateTime, Utc};
use sqlx::FromRow;

use crate::economy_http::{earn_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::games;

use super::pool::dberr;

/// 一个进行中 / 已结算的房间期。
#[derive(FromRow)]
pub(super) struct RoundRow {
    pub id: i64,
    pub status: String,
    pub dice: Option<Vec<i16>>,
    pub sum: Option<i16>,
    pub region: Option<String>,
    pub total_small: i64,
    pub total_big: i64,
    pub opened_at: DateTime<Utc>,
}

/// 我在某一期里的下注（结算后回填 payout/net）。
pub(super) struct MyBet {
    pub side: String,
    pub amount: i64,
    pub payout: i64,
    pub net: i64,
}

/// 上一期开奖结果（供舞台回放最后一骰）。
#[derive(FromRow)]
pub(super) struct LastRound {
    pub dice: Vec<i16>,
    pub sum: i16,
    pub region: String,
}

const ROUND_TABLE: &str = "arcade_bigsmall_rounds";
const BET_TABLE: &str = "arcade_bigsmall_bets";

async fn load_latest(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> DomainResult<Option<RoundRow>> {
    let row: Option<RoundRow> = sqlx::query_as(&format!(
        "SELECT id, status, dice, sum, region, total_small, total_big, \
             opened_at FROM {ROUND_TABLE} ORDER BY id DESC LIMIT 1 FOR UPDATE"
    ))
    .fetch_optional(&mut **tx)
    .await
    .map_err(dberr)?;
    Ok(row)
}

async fn insert_open(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> DomainResult<RoundRow> {
    let row: RoundRow = sqlx::query_as(&format!(
        "INSERT INTO {ROUND_TABLE} (status) VALUES ('betting') \
             RETURNING id, status, dice, sum, region, total_small, \
             total_big, opened_at"
    ))
    .fetch_one(&mut **tx)
    .await
    .map_err(dberr)?;
    Ok(row)
}

/// 结算某一期：掷骰定区，遍历本期下注派彩并回填 payout/net，把期次标为 settled。
async fn settle_round(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    round_id: i64,
    mult_permille: i64,
) -> DomainResult<(Vec<i16>, i16, String)> {
    let dice = games::roll();
    let region = games::region_of(dice);
    let sum: i16 = dice.iter().sum::<u32>() as i16;
    let dice_vec: Vec<i16> = dice.iter().map(|d| *d as i16).collect();

    let bets: Vec<(i64, i64, String, i64)> = sqlx::query_as(&format!(
        "SELECT id, user_id, side, amount FROM {BET_TABLE} \
             WHERE round_id = $1 FOR UPDATE"
    ))
    .bind(round_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(dberr)?;

    for (bet_id, uid, side, amount) in bets {
        let win = games::side_wins(region, &side);
        let payout = if win {
            games::payout(amount, mult_permille)
        } else {
            0
        };
        let net = payout - amount;
        if payout > 0 {
            let win_idem = format!("game-bs-round-win:{round_id}:{bet_id}");
            if !matches!(
                earn_spark_tx(tx, uid, payout, "game", &win_idem).await?,
                SpendOutcome::Spent
            ) {
                return Err(DomainError::LedgerConflict);
            }
        }
        sqlx::query(&format!(
            "UPDATE {BET_TABLE} SET payout=$1, net=$2 WHERE id=$3"
        ))
        .bind(payout)
        .bind(net)
        .bind(bet_id)
        .execute(&mut **tx)
        .await
        .map_err(dberr)?;
    }

    sqlx::query(&format!(
        "UPDATE {ROUND_TABLE} SET status = 'settled', dice = $1, sum = $2, \
             region = $3, settled_at = now() WHERE id = $4"
    ))
    .bind(&dice_vec)
    .bind(sum)
    .bind(region)
    .bind(round_id)
    .execute(&mut **tx)
    .await
    .map_err(dberr)?;

    Ok((dice_vec, sum, region.to_string()))
}

/// 推进到当前可下注期：过期则结算并开新期；无首期则创建。返回进行中的 betting 期。
pub(super) async fn advance_open(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    mult_permille: i64,
    window: i64,
) -> DomainResult<RoundRow> {
    let Some(latest) = load_latest(tx).await? else {
        return insert_open(tx).await;
    };

    if latest.status == "settled" {
        // 兜底：上一期已是结算态（例如首期之前被手动收尾），直接开新期。
        return insert_open(tx).await;
    }

    let elapsed = Utc::now()
        .signed_duration_since(latest.opened_at)
        .num_seconds();
    if elapsed < window as i64 {
        return Ok(latest);
    }

    settle_round(tx, latest.id, mult_permille).await?;
    insert_open(tx).await
}

/// 我在某一期的下注汇总。
pub(super) async fn my_bets(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    round_id: i64,
    user_id: i64,
) -> DomainResult<Vec<MyBet>> {
    let rows: Vec<(String, i64, i64, i64)> = sqlx::query_as(&format!(
        "SELECT side, amount, payout, net FROM {BET_TABLE} \
             WHERE round_id = $1 AND user_id = $2 ORDER BY id"
    ))
    .bind(round_id)
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(dberr)?;
    Ok(rows
        .into_iter()
        .map(|(side, amount, payout, net)| MyBet {
            side,
            amount,
            payout,
            net,
        })
        .collect())
}

/// 最近一期 settled 开奖结果（舞台回放）。
pub(super) async fn last_settled(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> DomainResult<Option<LastRound>> {
    let row: Option<LastRound> = sqlx::query_as(&format!(
        "SELECT dice, sum, region FROM {ROUND_TABLE} \
             WHERE status = 'settled' AND dice IS NOT NULL \
             ORDER BY id DESC LIMIT 1"
    ))
    .fetch_optional(&mut **tx)
    .await
    .map_err(dberr)?;
    Ok(row)
}

/// 本期一侧累计注额（下注后累加）。
pub(super) async fn add_side_total(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    round_id: i64,
    side: &str,
    amount: i64,
) -> DomainResult<()> {
    let col = if side == "small" {
        "total_small"
    } else {
        "total_big"
    };
    sqlx::query(&format!(
        "UPDATE {ROUND_TABLE} SET {col} = {col} + $1 WHERE id = $2"
    ))
    .bind(amount)
    .bind(round_id)
    .execute(&mut **tx)
    .await
    .map_err(dberr)?;
    Ok(())
}

/// 落一笔下注。幂等键冲突视为重放（调用方先靠 spend 幂等闸过一道）。
pub(super) async fn insert_bet(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    round_id: i64,
    user_id: i64,
    side: &str,
    amount: i64,
    idem: &str,
) -> DomainResult<()> {
    sqlx::query(&format!(
        "INSERT INTO {BET_TABLE} \
             (round_id, user_id, side, amount, idempotency_key) \
             VALUES ($1, $2, $3, $4, $5)"
    ))
    .bind(round_id)
    .bind(user_id)
    .bind(side)
    .bind(amount)
    .execute(&mut **tx)
    .await
    .map_err(|e| {
        // 唯一键（23505）撞 = 同一笔下注被重复提交；其余 DB 错误照常上抛，不吞成校验错
        let unique = e
            .as_database_error()
            .and_then(|d| d.code())
            .map(|c| c == "23505")
            .unwrap_or(false);
        if unique {
            DomainError::Validation("该注已受理，请勿重复提交".into())
        } else {
            dberr(e)
        }
    })?;
    Ok(())
}
