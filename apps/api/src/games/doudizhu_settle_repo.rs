//! 斗地主终局结算（事务内 SQL，从 doudizhu_repo.rs 拆出：主文件
//! 贴 300 行门禁）。倍数合成、抽水派彩、流局退款、终局列回填。

use super::doudizhu::DOU_DIZHU_RAKE;
use super::doudizhu_game::GameState;
use super::doudizhu_repo::GameRow;
use super::doudizhu_settle::{multiplier_of, settle_payout, Role};
use crate::economy_http::{earn_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};

fn dberr(e: sqlx::Error) -> DomainError {
    DomainError::Internal(anyhow::Error::from(e))
}

/// 抽水（千分比）：设置键 doudizhu_rake_permille（0521，0..500）同事务读；
/// 读失败/未配置回落代码常量（DOU_DIZHU_RAKE），行为与旧硬编码一致。
async fn rake_permille_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> i64 {
    match sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'doudizhu_rake_permille'), $1)",
    )
    .bind(DOU_DIZHU_RAKE)
    .fetch_one(&mut **tx)
    .await
    {
        Ok(v) => v.clamp(0, 500),
        Err(e) => {
            tracing::warn!(?e, "读斗地主抽水失败，回落常量");
            DOU_DIZHU_RAKE
        }
    }
}

/// 快照序列化（统一错误包装）。
fn snapshot_of(state: &GameState) -> DomainResult<serde_json::Value> {
    serde_json::to_value(state)
        .map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))
}

/// 终局结算：倍数合成、派彩（抽水后）、回填 DB 列与角色。
/// 流局（finished == u8::MAX）全退底分。
pub(crate) async fn settle_finished_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    row: &mut GameRow,
) -> DomainResult<()> {
    let finished = row.state.finished.unwrap_or(0);
    // 流局：底分全额退（同一事务内 earn 回来）
    if finished == u8::MAX {
        let refund_idem = format!("game-doudizhu-refund:{}", row.id);
        let refunded =
            earn_spark_tx(tx, uid, row.stake, "game", &refund_idem).await?;
        if matches!(refunded, SpendOutcome::Replayed) {
            tracing::warn!(gid = row.id, "doudizhu refund replayed");
        }
        let role: Option<String> = None;
        sqlx::query(
            "UPDATE arcade_doudizhu_games \
                SET status = 'settled', role = NULL, multiplier = 1, \
                    result = NULL, payout = $2, net = 0, \
                    state = $3, settled_at = now(), updated_at = now() \
              WHERE id = $1",
        )
        .bind(row.id)
        .bind(row.stake)
        .bind(snapshot_of(&row.state)?)
        .execute(&mut **tx)
        .await
        .map_err(dberr)?;
        row.role = role;
        row.multiplier = 1;
        row.result = None;
        row.payout = row.stake;
        row.net = 0;
        row.status = "settled".into();
        return Ok(());
    }
    let Some(landlord) = row.state.landlord else {
        return Err(DomainError::Internal(anyhow::anyhow!("终局无地主")));
    };
    // 胜负判定（经典口径）：地主出完=地主赢；任一农民出完=**两家农民
    // 同赢**——AI 房玩家当农民时队友先走完也算玩家赢（此前写 finished
    // ==0，队友赢玩家被判输，资深玩家一眼抓的结算 bug）。
    let player_is_landlord = landlord == 0;
    let player_won = if player_is_landlord {
        finished == 0
    } else {
        finished != landlord
    };
    let role = if player_is_landlord {
        Role::Landlord
    } else {
        Role::Farmer
    };
    let call_base: i64 = row
        .state
        .bids
        .iter()
        .find(|&&(s, _)| s == landlord)
        .map(|&(_, sc)| sc)
        .unwrap_or(1)
        .max(1);
    let spring = row.state.is_spring();
    let mult = multiplier_of(call_base, row.state.bombs, spring);
    let payout = if player_won {
        let rake = rake_permille_tx(tx).await;
        settle_payout(role, row.stake, mult, rake)
    } else {
        0
    };
    // 派彩（幂等键按局号派生，重放安全）
    if payout > 0 {
        let win_idem = format!("game-doudizhu-win:gid:{}", row.id);
        let paid = earn_spark_tx(tx, uid, payout, "game", &win_idem).await?;
        if matches!(paid, SpendOutcome::Replayed) {
            tracing::warn!(gid = row.id, "doudizhu payout replayed");
        }
    }
    let role_str = match role {
        Role::Landlord => "landlord",
        Role::Farmer => "farmer",
    };
    let result = if player_won { "win" } else { "lose" };
    let net = payout - row.stake;
    sqlx::query(
        "UPDATE arcade_doudizhu_games \
            SET status = 'settled', role = $2, multiplier = $3, \
                bomb_count = $4, spring = $5, result = $6, payout = $7, \
                net = $8, state = $9, settled_at = now(), updated_at = now() \
          WHERE id = $1 AND status != 'settled'",
    )
    .bind(row.id)
    .bind(role_str)
    .bind(mult)
    .bind(row.state.bombs as i32)
    .bind(spring)
    .bind(result)
    .bind(payout)
    .bind(net)
    .bind(snapshot_of(&row.state)?)
    .execute(&mut **tx)
    .await
    .map_err(dberr)?;
    row.role = Some(role_str.into());
    row.multiplier = mult;
    row.bomb_count = row.state.bombs as i32;
    row.spring = spring;
    row.result = Some(result.into());
    row.payout = payout;
    row.net = net;
    row.status = "settled".into();
    Ok(())
}
