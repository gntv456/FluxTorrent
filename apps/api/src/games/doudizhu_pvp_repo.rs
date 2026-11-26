//! 斗地主 PvP 房间仓储（事务内 SQL，从 doudizhu_repo 拆域）。
//!
//! 房间只管「撮合与座位」：waiting 等三真人 → 满员按座位建三行
//! `arcade_doudizhu_games`（每人一行、同一局组 = 同一房间 id 下的
//! 镜像牌局，state 各自独立存储），结算走既有 `settle_finished_tx`。
//!
//! 扣款时机：进房不扣，**房主开局那笔事务**里统一扣三人底分——
//! 任何一人余额不足则整桌散（先到先得的座位不占资金）。

use super::doudizhu_game::GameState;
use crate::economy_http::{spend_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};

fn dberr(e: sqlx::Error) -> DomainError {
    DomainError::Internal(anyhow::Error::from(e))
}

/// 快照序列化（与 repo 同款包装）。
fn snapshot_of(state: &GameState) -> DomainResult<serde_json::Value> {
    serde_json::to_value(state)
        .map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))
}

/// 建房（房主坐 0 号位）。
pub(crate) async fn create_room_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    stake: i64,
    variant: &str,
) -> DomainResult<i64> {
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO arcade_doudizhu_rooms \
            (variant, stake, status, owner_id) \
         VALUES ($1, $2, 'waiting', $3) RETURNING id",
    )
    .bind(variant)
    .bind(stake)
    .bind(uid)
    .fetch_one(&mut **tx)
    .await
    .map_err(dberr)?;
    // 房主直接坐 0 号位
    insert_seat_tx(tx, rid, 0, uid).await?;
    Ok(rid)
}

/// 坐座（含全局「一人一活跃房」与「同房一座」的索引兜底）。
pub(crate) async fn insert_seat_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    rid: i64,
    seat: i16,
    uid: i64,
) -> DomainResult<()> {
    sqlx::query(
        "INSERT INTO arcade_doudizhu_room_seats \
            (room_id, seat, user_id, ready, active) \
         VALUES ($1, $2, $3, false, true)",
    )
    .bind(rid)
    .bind(seat)
    .bind(uid)
    .execute(&mut **tx)
    .await
    .map_err(dberr)?;
    Ok(())
}

/// 匹配：找同底分同变体的 waiting 房间并占座；没有则建房。
/// 返回 (room_id, seat, joined_now)。
pub(crate) async fn join_or_create_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    stake: i64,
    variant: &str,
) -> DomainResult<(i64, i16, bool)> {
    // 已在活跃房：直接回房间（幂等刷新）
    let mine: Option<(i64, i16)> = sqlx::query_as(
        "SELECT s.room_id, s.seat \
           FROM arcade_doudizhu_room_seats s \
           JOIN arcade_doudizhu_rooms r ON r.id = s.room_id \
          WHERE s.user_id = $1 AND s.active \
            AND r.status IN ('waiting', 'playing') \
          ORDER BY s.room_id DESC LIMIT 1",
    )
    .bind(uid)
    .fetch_optional(&mut **tx)
    .await
    .map_err(dberr)?;
    if let Some((rid, seat)) = mine {
        return Ok((rid, seat, false));
    }
    // 找一个有空座的 waiting 房（FOR UPDATE 锁房行防并发超卖）
    let candidates: Vec<i64> = sqlx::query_scalar(
        "SELECT r.id FROM arcade_doudizhu_rooms r \
          WHERE r.variant = $1 AND r.stake = $2 AND r.status = 'waiting' \
          ORDER BY r.created_at LIMIT 5 FOR UPDATE",
    )
    .bind(variant)
    .bind(stake)
    .fetch_all(&mut **tx)
    .await
    .map_err(dberr)?;
    for rid in candidates {
        let taken: Vec<i16> = sqlx::query_scalar(
            "SELECT seat FROM arcade_doudizhu_room_seats \
              WHERE room_id = $1 ORDER BY seat",
        )
        .bind(rid)
        .fetch_all(&mut **tx)
        .await
        .map_err(dberr)?;
        if taken.len() >= 3 {
            continue;
        }
        // 占最小空座
        for seat in 0..3i16 {
            if !taken.contains(&seat) {
                insert_seat_tx(tx, rid, seat, uid).await?;
                return Ok((rid, seat, true));
            }
        }
    }
    // 没有可用房间 → 建房
    let rid = create_room_tx(tx, uid, stake, variant).await?;
    Ok((rid, 0, true))
}

/// 满员开局（房主触发，或加入后自动）：建三行镜像局 + 房转 playing。
/// 每座一行局（user_id 各异、state 同一副牌的三份独立快照）。
/// 任一座扣底分失败 → 整桌散（回滚 + 房转 abandoned）。
pub(crate) async fn deal_room_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    rid: i64,
) -> DomainResult<()> {
    let (variant, stake, status): (String, i64, String) = sqlx::query_as(
        "SELECT variant, stake, status FROM arcade_doudizhu_rooms \
          WHERE id = $1 FOR UPDATE",
    )
    .bind(rid)
    .fetch_one(&mut **tx)
    .await
    .map_err(dberr)?;
    if status != "waiting" {
        return Err(DomainError::Validation("房间已开局或已散".into()));
    }
    let seats: Vec<(i16, i64)> = sqlx::query_as(
        "SELECT seat, user_id FROM arcade_doudizhu_room_seats \
          WHERE room_id = $1 ORDER BY seat",
    )
    .bind(rid)
    .fetch_all(&mut **tx)
    .await
    .map_err(dberr)?;
    if seats.len() != 3 {
        return Err(DomainError::Validation("三人才可开局".into()));
    }
    // 经济闭环：**同一事务内**先扣三人底分，再建三行镜像局。任一座
    // 余额不足（InsufficientSpark）→ 整笔回滚（含已扣的座），房间停留
    // waiting，等补足再点开局；幂等键按房间+用户派生，重放安全。
    for (_seat, uid) in &seats {
        let idem = format!("game-dz-pvp:{}:{}", rid, uid);
        match spend_spark_tx(tx, *uid, stake, "game", &idem, "doudizhu", rid)
            .await?
        {
            SpendOutcome::Spent => {}
            // 同键已扣过（房主重试开局）：不重复扣，继续建局流程
            SpendOutcome::Replayed => {}
        }
    }
    // 发一副牌（三座共用）；不洗牌变体走聚类发牌。
    let (hands, kitty) = if variant == "nolook" {
        super::doudizhu::deal_nolook()
    } else {
        super::doudizhu::deal()
    };
    let is_wild = variant == "wild";
    let mut first_game_id: Option<i64> = None;
    for (idx, (_seat, uid)) in seats.iter().enumerate() {
        let my_hand = hands[idx].to_vec();
        let right = hands[(idx + 1) % 3].to_vec();
        let left = hands[(idx + 2) % 3].to_vec();
        let mut state = if is_wild {
            super::doudizhu_variant::new_wild(
                [my_hand.clone(), right, left],
                kitty.to_vec(),
            )
        } else {
            GameState::new([my_hand.clone(), right, left], kitty.to_vec())
        };
        // 镜像局协议第二半：物理首叫者是发牌序 0 的玩家；该玩家在
        // idx 行的座位 = (3 - idx) % 3。三行初始 turn 必须指向同一
        // 物理玩家，否则叫分顺序在三个视角里不一致，镜像重放全错位。
        state.turn = ((3 - idx) % 3) as u8;
        let snapshot = snapshot_of(&state)?;
        let idem = format!("game-dz-pvp:{}:{}", rid, uid);
        let gid: i64 = sqlx::query_scalar(
            "INSERT INTO arcade_doudizhu_games \
                (user_id, mode, variant, room_id, status, stake, \
                 idempotency_key, state) \
             VALUES ($1, 'pvp', $2, $3, 'bidding', $4, $5, $6) \
             RETURNING id",
        )
        .bind(uid)
        .bind(&variant)
        .bind(rid)
        .bind(stake)
        .bind(&idem)
        .bind(&snapshot)
        .fetch_one(&mut **tx)
        .await
        .map_err(dberr)?;
        if first_game_id.is_none() {
            first_game_id = Some(gid);
        }
    }
    sqlx::query(
        "UPDATE arcade_doudizhu_rooms \
            SET status = 'playing', game_id = $2, updated_at = now() \
          WHERE id = $1",
    )
    .bind(rid)
    .bind(first_game_id)
    .execute(&mut **tx)
    .await
    .map_err(dberr)?;
    Ok(())
}

/// 离开/散桌：waiting 房间走人即退座（房主走则整桌散）；
/// 活跃座位置 false 释放「一人一活跃房」。
pub(crate) async fn leave_room_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
) -> DomainResult<Option<i64>> {
    let Some((rid, seat)): Option<(i64, i16)> = sqlx::query_as(
        "SELECT s.room_id, s.seat \
           FROM arcade_doudizhu_room_seats s \
           JOIN arcade_doudizhu_rooms r ON r.id = s.room_id \
          WHERE s.user_id = $1 AND s.active AND r.status = 'waiting' \
          ORDER BY s.room_id DESC LIMIT 1",
    )
    .bind(uid)
    .fetch_optional(&mut **tx)
    .await
    .map_err(dberr)?
    else {
        return Ok(None);
    };
    let (owner_id,): (i64,) = sqlx::query_as(
        "SELECT owner_id FROM arcade_doudizhu_rooms WHERE id = $1",
    )
    .bind(rid)
    .fetch_one(&mut **tx)
    .await
    .map_err(dberr)?;
    // 房主离开 → 整桌散
    if owner_id == uid {
        sqlx::query(
            "UPDATE arcade_doudizhu_rooms \
                SET status = 'abandoned', updated_at = now() WHERE id = $1",
        )
        .bind(rid)
        .execute(&mut **tx)
        .await
        .map_err(dberr)?;
        sqlx::query(
            "UPDATE arcade_doudizhu_room_seats SET active = false \
              WHERE room_id = $1",
        )
        .bind(rid)
        .execute(&mut **tx)
        .await
        .map_err(dberr)?;
        return Ok(Some(rid));
    }
    // 普通成员退座；该座释放
    sqlx::query(
        "DELETE FROM arcade_doudizhu_room_seats \
          WHERE room_id = $1 AND seat = $2",
    )
    .bind(rid)
    .bind(seat)
    .execute(&mut **tx)
    .await
    .map_err(dberr)?;
    Ok(Some(rid))
}

/// 切换本人准备状态（waiting 房内有效）；同时刷新 last_seen。
pub(crate) async fn set_ready_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    ready: bool,
) -> DomainResult<()> {
    sqlx::query(
        "UPDATE arcade_doudizhu_room_seats \
            SET ready = $2, last_seen = now() \
          WHERE user_id = $1 AND active",
    )
    .bind(uid)
    .bind(ready)
    .execute(&mut **tx)
    .await
    .map_err(dberr)?;
    Ok(())
}

/// 刷新本人活跃座 last_seen（心跳：进房/看房间/动作时调用）。
pub(crate) async fn touch_seen_db(
    db: &sqlx::PgPool,
    uid: i64,
) -> DomainResult<()> {
    sqlx::query(
        "UPDATE arcade_doudizhu_room_seats SET last_seen = now() \
          WHERE user_id = $1 AND active",
    )
    .bind(uid)
    .execute(db)
    .await
    .map_err(dberr)?;
    Ok(())
}
