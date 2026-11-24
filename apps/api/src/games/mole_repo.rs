//! 打地鼠结算仓储（事务内 SQL，从 games_http/mole_settle.rs 下沉：H 层
//! 禁 SQL 棘轮）。纯 DB 动作，不含 HTTP 语义；入口仍是
//! games_http/mole_settle.rs 的 handler。

use crate::economy_http::{earn_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::games;

fn dberr(e: sqlx::Error) -> DomainError {
    DomainError::Internal(anyhow::Error::from(e))
}

/// 结算一局的资金段（事务内）：入账 → 标记 resolved → 逐档落图鉴。
///
/// 调用方必须已确认这一局**排程走完且未结算**。`mole_finish` 与
/// `mole_start` 的补结算共用它 —— 单局结算纪律只有这一份。
/// 并发双击时输掉的那一笔返回 `false`（调用方回滚自己的事务即可）。
pub(crate) async fn settle_round_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    round_id: i64,
    idem: &str,
    hits: &[(i32, i64)],
) -> DomainResult<(i64, i64)> {
    let values: Vec<i64> = hits.iter().map(|(_, v)| *v).collect();
    let (score, payout) = games::settle_score(&values);
    let win_idem = format!("game-mole-win:{idem}");
    if payout > 0 {
        let paid = earn_spark_tx(tx, uid, payout, "game", &win_idem).await?;
        // 上一笔已入账但没来得及标记 resolved（进程中断）时会重放：
        // 钱已经在账上，这里继续收尾即可，不能因为重放就跳过结算
        if matches!(paid, SpendOutcome::Replayed) {
            tracing::warn!(round_id, "mole payout replayed");
        }
    }
    let marked = sqlx::query(
        "UPDATE arcade_mole_rounds SET resolved = true, score = $2, \
                payout = $3, resolved_at = now() \
          WHERE id = $1 AND NOT resolved",
    )
    .bind(round_id)
    .bind(score)
    .bind(payout)
    .execute(&mut **tx)
    .await
    .map_err(dberr)?;
    if marked.rows_affected() == 0 {
        // 并发双击 finish：让对方那一笔赢
        return Err(DomainError::Validation("这一局已结算，请刷新战绩".into()));
    }
    // 图鉴/稀有播报的事实源：每只中奖地鼠一行（炸弹与空档不记）
    record_tiers(tx, uid, round_id, hits).await?;
    Ok((score, payout))
}

/// 把这一用户**排程已走完但没结算**的旧局逐个补结算（事务内）。
///
/// 用在 `mole_start`：以前切标签页会当场把半局结算掉；现在提前结算被
/// `schedule_done` 挡下，旧局留在未结算态 —— 玩家不回来领就一直是笔坏账。
/// 开新局前先把它们结清（命中的分数照付），再由调用方清理。
pub(crate) async fn settle_stale_rounds_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
) -> DomainResult<usize> {
    let stale: Vec<(i64, i64, String, serde_json::Value, i64)> =
        sqlx::query_as(
            "SELECT id, bet, COALESCE(idem, ''), schedule, \
                    (EXTRACT(EPOCH FROM (now() - created_at)) * 1000)::bigint \
               FROM arcade_mole_rounds \
              WHERE user_id = $1 AND NOT resolved \
                AND created_at < now() - make_interval(secs => $2) \
              ORDER BY id",
        )
        .bind(uid)
        .bind((games::ROUND_MS + games::HIT_LATE_MS + 5_000) / 1000)
        .fetch_all(&mut **tx)
        .await
        .map_err(dberr)?;
    let mut done = 0;
    for (round_id, _bet, idem, schedule, elapsed) in stale {
        let spawns: Vec<games::Spawn> =
            serde_json::from_value(schedule).unwrap_or_default();
        // 时间闸只是粗筛（排程长度固定，粗筛窗口已留足余量）；这里按
        // 「排程走完 或 整局时长已过」精确复核，没走完的一律不碰。
        if !games::settled_or_over(&spawns, elapsed) {
            continue;
        }
        let hits: Vec<(i32, i64)> = sqlx::query_as(
            "SELECT seq, value FROM arcade_mole_hits WHERE round_id = $1",
        )
        .bind(round_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(dberr)?;
        match settle_round_tx(tx, uid, round_id, &idem, &hits).await {
            Ok(_) => done += 1,
            // 输给并发的另一笔 finish：那一局已结清，不算失败
            Err(DomainError::Validation(_)) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(done)
}

/// 逐档落 arcade_pool_rounds（rarity/标签取当前池；值取排程，防池子中途改值）。
async fn record_tiers(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    round_id: i64,
    hits: &[(i32, i64)],
) -> DomainResult<()> {
    let schedule: serde_json::Value = sqlx::query_scalar(
        "SELECT schedule FROM arcade_mole_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(dberr)?;
    let spawns: Vec<games::Spawn> =
        serde_json::from_value(schedule).unwrap_or_default();
    for (seq, value) in hits {
        if *value <= 0 {
            continue;
        }
        let Some(s) = spawns.iter().find(|s| s.seq == *seq) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO arcade_pool_rounds \
                (user_id, game, entry_index, rarity, prize, spark, value) \
             VALUES ($1, 'mole', $2, $3, $4, $5, $5)",
        )
        .bind(uid)
        .bind(i64::from(s.idx))
        .bind(s.rarity)
        .bind(&s.label)
        .bind(value)
        .execute(&mut **tx)
        .await
        .map_err(dberr)?;
    }
    Ok(())
}
