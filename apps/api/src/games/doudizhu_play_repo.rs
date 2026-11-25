//! 斗地主 AI 段推进与快照落库（从 doudizhu_repo.rs 拆出守 300 行门禁）。
//! 玩家动作后由 repo 层调 `run_ai_segments` 走完 AI 叫分/出牌，终局交接结算。

use super::doudizhu_ai::{ai_bid, ai_choose};
use super::doudizhu_game::{GameState, Phase};
use super::doudizhu_repo::GameRow;
use crate::errors::{DomainError, DomainResult};
fn dberr(e: sqlx::Error) -> DomainError {
    DomainError::Internal(anyhow::Error::from(e))
}

/// PvP 超时懒托管：本局轮到玩家（seat0）且 `updated_at` 已停摆超过
/// `idle_secs` → AI 代打到底（含结算）。AI 房不受影响（AI 段本来就由
/// run_ai_segments 即时推进，轮到玩家时 updated_at 也早已刷新）。
///
/// 判定口径：`now() - updated_at > idle_secs`。updated_at 在每次
/// persist/settle 都刷新，PvP 三行镜像局各自独立计时——任一座挂机，
/// 其它两座任何动作/读局都会触发代打，整桌不会停摆。
/// `idle_secs <= 0` = 关闭（start_game_tx 内部重放路径用）。
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
        if row.state.phase == Phase::Bidding {
            let suggested = GameState::suggested_bid(&row.state.hands[0]);
            let high = row.state.bids.iter().map(|b| b.1).max().unwrap_or(0);
            row.state
                .bid(0, ai_bid(suggested, high))
                .map_err(|e| DomainError::Validation(e.to_string()))?;
        } else {
            let seat = 0u8;
            let hand = row.state.hands[0].clone();
            let pick = match (&row.state.last, row.state.last_seat) {
                (Some(c), Some(s)) => {
                    let prev = c.clone();
                    let l = row.state.landlord;
                    ai_choose(&hand, Some((&prev, s)), seat, false, l)
                }
                _ => ai_choose(&hand, None, seat, false, row.state.landlord),
            };
            // 癞子座退化同 run_ai_segments（出最小非癞子牌）
            let cards = pick
                .or_else(|| {
                    let wilds = &row.state.wildcards[0];
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
                })
                .unwrap_or_default();
            // 领出必须出牌；候选被拒（癞子面外的边角）则降级过牌
            if cards.is_empty() && row.state.last.is_none() {
                break; // 无牌可领的保险出口（手牌空不应到这）
            }
            if row.state.play(seat, &cards).is_err() && !cards.is_empty() {
                let _ = row.state.play(seat, &[]);
            }
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

/// expire_stale 专用行加载（FOR UPDATE 已在外层持锁，此处只读快照）。
async fn load_row_by(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    gid: i64,
) -> DomainResult<GameRow> {
    let row = sqlx::query_as::<_, super::doudizhu_repo::DbRow>(
        "SELECT id, status, stake, role, multiplier, bomb_count, \
                spring, result, payout, net, state, room_id \
           FROM arcade_doudizhu_games \
          WHERE id = $1 AND user_id = $2",
    )
    .bind(gid)
    .bind(uid)
    .fetch_optional(&mut **tx)
    .await
    .map_err(dberr)?;
    row.map(super::doudizhu_repo::decode_row)
        .transpose()?
        .ok_or_else(|| DomainError::Validation("对局不存在".into()))
}

/// PvP 镜像同步：把本行新事件按座位变换重放到同房另两行。
/// 行 X(idx) 里物理玩家座位 = (P_idx - idx + 3) % 3；重放走各自状态机，
/// 失败只 warn（发起行权威，掉队行靠 expire_stale_tx 收敛）；重放后
/// phase=Done 且未结算的行走 settle_finished_tx 各自派彩。
pub(crate) async fn sync_mirror_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    origin: &GameRow,
    new_events_from: usize,
) -> DomainResult<()> {
    let Some(room_id) = origin.room_id else {
        return Ok(()); // AI 局无镜像
    };
    // 发起行在房内的发牌序：镜像局建局时按座位序插入，行 id 递增，
    // 取同房三行按 id 升序即发牌序。
    let ids: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, user_id FROM arcade_doudizhu_games \
          WHERE room_id = $1 ORDER BY id",
    )
    .bind(room_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(dberr)?;
    let origin_idx = ids.iter().position(|(id, _)| *id == origin.id);
    let Some(origin_idx) = origin_idx else {
        return Ok(());
    };
    let new_events: Vec<super::doudizhu_game::Event> = origin
        .state
        .events
        .iter()
        .skip(new_events_from)
        .cloned()
        .collect();
    // 死锁防护：同时动作互扇会锁对方行，按 peer id 升序（一致锁序）。
    let mut peers: Vec<(i64, i64, usize)> = ids
        .iter()
        .enumerate()
        .filter(|(_, (id, _))| id != &origin.id)
        .map(|(idx, (id, uid))| (*id, *uid, idx))
        .collect();
    peers.sort_by_key(|(id, _, _)| *id);
    for (_peer_i, (peer_gid, peer_uid, peer_idx)) in
        peers.iter().enumerate().map(|(i, p)| (i, *p))
    {
        // 座位变换：origin 行 seat=s → peer 行 seat=(s + peer_idx -
        // origin_idx + 3) % 3
        // 变换方向：origin 行座位 s 的物理座位 = (s + origin_idx) % 3，
        // 在 peer 行座位 = (物理 - peer_idx + 3) % 3 = (s + origin_idx -
        // peer_idx + 3) % 3（此前 peer/origin 写反，镜像行永远不同步）。
        let shift = (3 + origin_idx + 3 - peer_idx) % 3;
        let mut row =
            match super::doudizhu_repo::load_by_id_tx(tx, peer_uid, peer_gid)
                .await
            {
                Ok(r) => r,
                Err(_) => continue,
            };
        for ev in &new_events {
            let seat = ((ev.seat as usize) + shift) % 3;
            let res = match ev.action.as_str() {
                "bid" => row.state.bid(seat as u8, ev.score.unwrap_or(0)),
                "play" => row.state.play(seat as u8, &ev.cards),
                _ => Ok(()),
            };
            if let Err(e) = res {
                tracing::warn!(
                    gid = row.id, seat, action = %ev.action,
                    err = e, "mirror replay skipped"
                );
            }
            if row.state.phase == Phase::Done {
                break;
            }
        }
        if row.state.phase == Phase::Done && row.status != "settled" {
            super::doudizhu_settle_repo::settle_finished_tx(
                tx, peer_uid, &mut row,
            )
            .await?;
        } else {
            super::doudizhu_play_store::persist_state_tx(tx, &mut row).await?;
        }
    }
    Ok(())
}

/// 推进所有非玩家段（AI 叫分/AI 出牌）；玩家轮到或终局即停。
/// 终局时落结算（派彩 + 回填 DB 列）。返回后仍需调用方提交。
pub(crate) async fn run_ai_segments(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    row: &mut GameRow,
) -> DomainResult<()> {
    if row.state.phase == Phase::Done {
        return Ok(());
    }
    while row.state.phase == Phase::Bidding && row.state.turn != 0 {
        let suggested =
            GameState::suggested_bid(&row.state.hands[row.state.turn as usize]);
        let seat = row.state.turn;
        let high = row.state.bids.iter().map(|b| b.1).max().unwrap_or(0);
        row.state
            .bid(seat, ai_bid(suggested, high))
            .map_err(|e| DomainError::Validation(e.to_string()))?;
    }
    // AI 出牌
    while row.state.phase == Phase::Playing && row.state.turn != 0 {
        let seat = row.state.turn;
        let hand = row.state.hands[seat as usize].clone();
        let pick = match (&row.state.last, row.state.last_seat) {
            (Some(c), Some(s)) => {
                // 农民互为队友：另一 AI 手牌 ≤4 时不抢
                let low = teammate_of(seat, row.state.landlord)
                    .map(|t| row.state.hands[t as usize].len() <= 4)
                    .unwrap_or(false);
                let l = row.state.landlord;
                ai_choose(&hand, Some((c, s)), seat, low, l)
            }
            _ => ai_choose(&hand, None, seat, false, row.state.landlord),
        };
        // 癞子座兜底出最小单张；领出必须出牌（全癞子手也出一张），否则卡死。
        let is_lead = row.state.last.is_none();
        let wilds = &row.state.wildcards[seat as usize];
        let cards = pick
            .or_else(|| {
                hand.iter()
                    .copied()
                    .filter(|c| wilds.is_empty() || !wilds.contains(c))
                    .min_by_key(|c| {
                        std::cmp::Reverse(super::doudizhu::Rank::of(*c))
                    })
                    .or_else(|| hand.first().copied())
                    .map(|c| vec![c])
            })
            .unwrap_or_default();
        // 领出被拒则降级单张；跟牌压不过则过牌（不抛错不回滚）
        if row.state.play(seat, &cards).is_err() {
            let retry = if is_lead {
                hand.first().map(|&x| vec![x]).unwrap_or_default()
            } else {
                vec![]
            };
            if row.state.play(seat, &retry).is_err() && is_lead {
                let _ = row.state.play(seat, &[]);
            }
        }
    }
    // 终局结算
    if row.state.phase == Phase::Done {
        super::doudizhu_settle_repo::settle_finished_tx(tx, uid, row).await?;
    } else {
        super::doudizhu_play_store::persist_state_tx(tx, row).await?;
    }
    Ok(())
}

/// seat 的农民队友（按真实阵营，非座位号）；地主无队友。
fn teammate_of(seat: u8, landlord: Option<u8>) -> Option<u8> {
    let l = landlord?;
    if l == seat {
        return None;
    }
    (0..3u8).find(|&s| s != l && s != seat)
}
