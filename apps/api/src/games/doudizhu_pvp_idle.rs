//! 斗地主 PvP 超时懒托管（从 doudizhu_play_repo.rs 拆出守 300 行门禁）。
//! 玩家停摆超 idle_secs → AI 代打到底；经典座优先 DouZero 模型建议
//! （dz_choose_safe 预校验+启发式兜底），癞子座维持启发式+最小非癞子牌。

use super::doudizhu_ai::{ai_bid, ai_choose};
use super::doudizhu_game::{GameState, Phase};
use super::doudizhu_play_repo::{load_row_by, run_ai_segments};
use crate::errors::{DomainError, DomainResult};

fn dberr(e: sqlx::Error) -> DomainError {
    DomainError::Internal(anyhow::Error::from(e))
}

pub(crate) async fn expire_stale_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    gid: i64,
    idle_secs: i64,
) -> DomainResult<()> {
    if idle_secs <= 0 {
        return Ok(());
    }
    let stale: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM arcade_doudizhu_games \
          WHERE id = $1 AND user_id = $2 \
            AND status IN ('bidding', 'playing') \
            AND now() - updated_at > make_interval(secs => $3) \
          FOR UPDATE",
    )
    .bind(gid)
    .bind(uid)
    .bind(idle_secs)
    .fetch_optional(&mut **tx)
    .await
    .map_err(dberr)?;
    let Some((gid,)) = stale else {
        return Ok(()); // 没停摆：正常返回
    };
    tracing::warn!(gid, uid, idle_secs, "doudizhu idle auto-play kicks in");
    let mut row = load_row_by(tx, uid, gid).await?;
    // 玩家座整段交给 AI：临时按 AI 视角跑 run_ai_segments——它只推
    // turn != 0 的段，所以先把玩家座「托管标记」下的所有轮次逐个代打：
    // 循环跑 run_ai_segments + 手动为 seat0 出一手，直至终局。
    let mut guard = 0;
    while row.state.phase != Phase::Done && row.state.turn == 0 {
        guard += 1;
        if guard > 500 {
            return Err(DomainError::Internal(anyhow::anyhow!(
                "托管代打死循环"
            )));
        }
        match row.state.phase {
            Phase::Bidding => {
                let suggested = GameState::suggested_bid(&row.state.hands[0]);
                let high =
                    row.state.bids.iter().map(|b| b.1).max().unwrap_or(0);
                row.state
                    .bid(0, ai_bid(suggested, high))
                    .map_err(|e| DomainError::Validation(e.to_string()))?;
            }
            Phase::Grab => {
                let take = GameState::suggested_bid(&row.state.hands[0]) >= 2;
                row.state
                    .grab(0, take)
                    .map_err(|e| DomainError::Validation(e.to_string()))?;
            }
            Phase::Discard => {
                row.state
                    .discard(0, &[])
                    .map_err(|e| DomainError::Validation(e.to_string()))?;
            }
            Phase::Double => {
                let s = GameState::suggested_bid(&row.state.hands[0]);
                let level = if s >= 3 {
                    2
                } else if s >= 2 {
                    1
                } else {
                    0
                };
                row.state
                    .double(0, level)
                    .map_err(|e| DomainError::Validation(e.to_string()))?;
            }
            Phase::Playing => {
                let seat = 0u8;
                let hand = row.state.hands[0].clone();
                // 经典座优先 DouZero 模型建议（dz_choose_safe 内含状态机
                // 口径预校验 + 失败回落启发式）；癞子座维持启发式。
                let wilds = &row.state.wildcards[0];
                let pick = if wilds.is_empty() {
                    super::doudizhu_dz::dz_choose_safe(&row.state, seat, false)
                } else {
                    match (&row.state.last, row.state.last_seat) {
                        (Some(c), Some(s)) => ai_choose(
                            &hand,
                            Some((c, s)),
                            seat,
                            false,
                            row.state.landlord,
                        ),
                        _ => ai_choose(
                            &hand,
                            None,
                            seat,
                            false,
                            row.state.landlord,
                        ),
                    }
                };
                // 癞子座退化同 run_ai_segments（出最小非癞子牌）
                let cards = pick.or_else(|| {
                    if wilds.is_empty() {
                        return None;
                    }
                    hand.iter()
                        .copied()
                        .filter(|c| !wilds.contains(c))
                        .min_by_key(|c| {
                            std::cmp::Reverse(super::doudizhu::Rank::of(*c))
                        })
                        .map(|c| vec![c])
                });
                let cards = cards.unwrap_or_default();
                // 领出必须出牌：候选被拒（模型建议与实体面错位等）→ 最小单张
                // 兜底，绝不允许空手领出——否则 state.play 报「领出不能过牌」
                // 被 ok() 吞掉，while 不动 → guard 500 → 事务回滚 → updated_at
                // 不刷 → 托管每 120s 重来且永远卡在同一步（dev.49 好学站
                // gid=6 实测 38 次 kicks in 的确定性死锁）。
                let cards = if cards.is_empty() && row.state.last.is_none() {
                    hand.iter()
                        .copied()
                        .min_by_key(|c| {
                            std::cmp::Reverse(super::doudizhu::Rank::of(*c))
                        })
                        .map(|c| vec![c])
                        .unwrap_or_default()
                } else {
                    cards
                };
                if cards.is_empty() {
                    break; // 手牌空（不应到这）
                }
                if row.state.play(seat, &cards).is_err() && !cards.is_empty() {
                    let _ = row.state.play(seat, &[]);
                }
            }
            Phase::Done => break,
        }
        // 推进对手段
        run_ai_segments(tx, uid, &mut row).await?;
    }
    if row.state.phase == Phase::Done {
        super::doudizhu_settle_repo::settle_finished_tx(tx, uid, &mut row)
            .await?;
    } else {
        super::doudizhu_play_store::persist_state_tx(tx, &mut row).await?;
    }
    Ok(())
}
