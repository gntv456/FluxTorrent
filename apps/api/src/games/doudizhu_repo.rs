//! 斗地主对局仓储（事务内 SQL）。H 层禁 SQL 棘轮——handler 只做鉴权与
//! 频控，动账与状态落库全部在这层（模式仿 `mole_repo.rs`）。
//!
//! 事务边界（钓场同款纪律）：开局扣底分 + 建局一笔提交；动作推进 + 终局
//! 结算一笔提交。玩家超时不另起 worker：`expire_stale_tx` 懒托管——
//! `turn_deadline` 过期就按 AI 到底打完并结算。

use super::doudizhu::{deal, deal_nolook};
use super::doudizhu_game::{GameState, Phase};
use crate::economy_http::{spend_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};

fn dberr(e: sqlx::Error) -> DomainError {
    DomainError::Internal(anyhow::Error::from(e))
}

/// 行元组别名：`(id, status, stake, role, mult, bombs, spring, result,
/// payout, net, state_json)`。multiplier 列是 INT4 → i32（sqlx 严格解码）。
pub(crate) type DbRow = (
    i64,
    String,
    i64,
    Option<String>,
    i32,
    i32,
    bool,
    Option<String>,
    i64,
    i64,
    serde_json::Value,
    Option<i64>, // room_id（PvP 镜像局共享；AI 局 NULL）
);

const ROW_COLS: &str = "id, status, stake, role, multiplier, bomb_count, \
                spring, result, payout, net, state, room_id";

/// 一局的持久化行（HTTP 层转 JSON 给前端）。
pub(crate) struct GameRow {
    pub id: i64,
    pub status: String,
    pub stake: i64,
    pub role: Option<String>,
    pub multiplier: i64,
    pub bomb_count: i32,
    pub spring: bool,
    pub result: Option<String>,
    pub payout: i64,
    pub net: i64,
    pub state: GameState,
    /// 玩家该不该动作（bid/play），HTTP 层换算成倒计时。
    pub my_turn: bool,
    /// 所属 PvP 房间（镜像局共享）；AI 局 None。
    pub room_id: Option<i64>,
}

/// 读取玩家当前活跃局（bidding/playing）；没有则 None。
pub(crate) async fn load_active(
    pool: &sqlx::PgPool,
    uid: i64,
) -> DomainResult<Option<GameRow>> {
    let sql = format!(
        "SELECT {ROW_COLS} FROM arcade_doudizhu_games \
         WHERE user_id = $1 AND status IN ('bidding', 'playing') \
         ORDER BY id DESC LIMIT 1"
    );
    let row: Option<DbRow> = sqlx::query_as(&sql)
        .bind(uid)
        .fetch_optional(pool)
        .await
        .map_err(dberr)?;
    row.map(decode_row).transpose()
}

/// 观战回显：按房间取一行活跃镜像局作权威进度（只读，不持行锁）。
/// 三行是同一局的不同视角，取最早一行即可；手牌在 HTTP 层按旁观脱敏。
pub(crate) async fn load_room_game_db(
    db: &sqlx::PgPool,
    rid: i64,
) -> DomainResult<Option<GameRow>> {
    let row: Option<DbRow> = sqlx::query_as(&format!(
        "SELECT {ROW_COLS} FROM arcade_doudizhu_games \
         WHERE room_id = $1 AND status IN ('bidding', 'playing') \
         ORDER BY id LIMIT 1"
    ))
    .bind(rid)
    .fetch_optional(db)
    .await
    .map_err(dberr)?;
    row.map(decode_row).transpose()
}

/// 读最近一局（含已结算）——战绩/复盘口用。
pub(crate) async fn load_last(
    pool: &sqlx::PgPool,
    uid: i64,
) -> DomainResult<Option<GameRow>> {
    let sql = format!(
        "SELECT {ROW_COLS} FROM arcade_doudizhu_games \
         WHERE user_id = $1 ORDER BY id DESC LIMIT 1"
    );
    let row: Option<DbRow> = sqlx::query_as(&sql)
        .bind(uid)
        .fetch_optional(pool)
        .await
        .map_err(dberr)?;
    row.map(decode_row).transpose()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn decode_row(r: DbRow) -> DomainResult<GameRow> {
    let state: GameState = serde_json::from_value(r.10)
        .map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;
    let my_turn = state.phase != Phase::Done && state.turn == 0;
    Ok(GameRow {
        id: r.0,
        status: r.1,
        stake: r.2,
        role: r.3,
        multiplier: i64::from(r.4),
        bomb_count: r.5,
        spring: r.6,
        result: r.7,
        payout: r.8,
        net: r.9,
        my_turn,
        state,
        room_id: r.11,
    })
}

/// 开局（事务内）：扣底分 + 建局。同一客户端幂等键重放时返回已有局。
pub(crate) async fn start_game_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    stake: i64,
    idem: &str,
    variant: &str,
    reveal: bool,
) -> DomainResult<GameRow> {
    // 幂等重放：同一键已开局则直接还给那一局
    let replay: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM arcade_doudizhu_games WHERE idempotency_key = $1",
    )
    .bind(idem)
    .fetch_optional(&mut **tx)
    .await
    .map_err(dberr)?;
    if let Some((gid,)) = replay {
        return fetch_game_tx(tx, uid, gid, 0).await;
    }
    // 扣底分（含余额锁与幂等检查；InsufficientSpark 由 spend 层给出）
    match spend_spark_tx(tx, uid, stake, "game", idem, "doudizhu", 0).await? {
        SpendOutcome::Spent => {}
        SpendOutcome::Replayed => {
            // 极小窗：同键并发开局。回查那一局
            let (gid,): (i64,) = sqlx::query_as(
                "SELECT id FROM arcade_doudizhu_games \
                 WHERE idempotency_key = $1",
            )
            .bind(idem)
            .fetch_one(&mut **tx)
            .await
            .map_err(dberr)?;
            return fetch_game_tx(tx, uid, gid, 0).await;
        }
    }
    // 发牌建局：癞子桌按 variant 选 new_wild（此前 variant 参数挂着
    // 却无条件用经典初始化，玩家选癞子实际拿到经典局）。
    let (hands, kitty) = if variant == "nolook" {
        deal_nolook()
    } else {
        deal()
    };
    let hands_vec = [hands[0].to_vec(), hands[1].to_vec(), hands[2].to_vec()];
    let mut state = if variant == "wild" {
        super::doudizhu_variant::new_wild(hands_vec, kitty.to_vec())
    } else {
        super::doudizhu_variant::new_nolook(hands_vec, kitty.to_vec())
    };
    state.reveal = reveal;
    let snapshot = snapshot_of(&state)?;
    let gid: i64 = sqlx::query_scalar(
        "INSERT INTO arcade_doudizhu_games \
            (user_id, mode, variant, status, stake, idempotency_key, state) \
         VALUES ($1, 'ai', $2, 'bidding', $3, $4, $5) RETURNING id",
    )
    .bind(uid)
    .bind(variant)
    .bind(stake)
    .bind(idem)
    .bind(&snapshot)
    .fetch_one(&mut **tx)
    .await
    .map_err(dberr)?;
    fetch_game_tx(tx, uid, gid, 0).await
}

/// 取局并按当前阶段推进 AI（不出玩家轮次）。
pub(crate) async fn fetch_game_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    gid: i64,
    idle_secs: i64,
) -> DomainResult<GameRow> {
    // 懒托管入口 2：读局面即触发（玩家刷新页面就能看到代打结果）
    super::doudizhu_play_repo::expire_stale_tx(tx, uid, gid, idle_secs).await?;
    let mut row = load_by_id_tx(tx, uid, gid).await?;
    super::doudizhu_play_repo::run_ai_segments(tx, uid, &mut row).await?;
    Ok(row)
}

/// 测试与镜像同步用的行加载（FOR UPDATE）。
pub(crate) async fn load_by_id_public(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    gid: i64,
) -> DomainResult<GameRow> {
    load_by_id_tx(tx, uid, gid).await
}

pub(crate) async fn load_by_id_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    gid: i64,
) -> DomainResult<GameRow> {
    let row = sqlx::query_as::<_, DbRow>(
        "SELECT id, status, stake, role, multiplier, bomb_count, \
                spring, result, payout, net, state, room_id \
           FROM arcade_doudizhu_games \
          WHERE id = $1 AND user_id = $2 FOR UPDATE",
    )
    .bind(gid)
    .bind(uid)
    .fetch_optional(&mut **tx)
    .await
    .map_err(dberr)?;
    row.map(decode_row)
        .transpose()?
        .ok_or_else(|| DomainError::Validation("对局不存在".into()))
}

/// 快照序列化（统一错误包装）。
fn snapshot_of(state: &GameState) -> DomainResult<serde_json::Value> {
    serde_json::to_value(state)
        .map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))
}

/// 玩家对局统计（段位派生用，只读 settled 局）。流局 result=NULL 不计胜负。
pub(crate) struct RankSummary {
    pub plays: i64,
    pub wins: i64,
    pub losses: i64,
    pub net: i64,
}

pub(crate) async fn rank_summary(
    pool: &sqlx::PgPool,
    uid: i64,
) -> DomainResult<RankSummary> {
    let row: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT count(*)::bigint, \
                COALESCE(sum(CASE WHEN result = 'win' THEN 1 ELSE 0 END), 0)::bigint, \
                COALESCE(sum(CASE WHEN result = 'lose' THEN 1 ELSE 0 END), 0)::bigint, \
                COALESCE(sum(net), 0)::bigint \
         FROM arcade_doudizhu_games \
         WHERE user_id = $1 AND status = 'settled'",
    )
    .bind(uid)
    .fetch_one(pool)
    .await
    .map_err(dberr)?;
    Ok(RankSummary {
        plays: row.0,
        wins: row.1,
        losses: row.2,
        net: row.3,
    })
}

/// 玩家动作（bid/play）入口：先懒托管（挂机代打），再校验 + 推进
/// + AI + 落库。
pub(crate) async fn apply_action_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    gid: i64,
    action: &str,
    score: Option<i64>,
    cards: &[u8],
    idle_secs: i64,
) -> DomainResult<GameRow> {
    // 懒托管入口 1：本局轮到玩家但停摆超阈值 → AI 代打到底。
    // kicked = 本次确实代打了（含把局推过用户动作所在阶段甚至终局结算）。
    let kicked =
        super::doudizhu_play_repo::expire_stale_tx(tx, uid, gid, idle_secs)
            .await?;
    let mut row = load_by_id_tx(tx, uid, gid).await?;
    // 托管完成后：Done 直接透传结果；未到 Done 但阶段已离开用户动作
    // （如 bid 到达时已 playing）——迟到动作作废，透传托管后局面。
    // 否则 my_turn/阶段闸会把代打成果连同事务一起回滚，玩家白丢胜负
    // （dev.49 百灵 gid=1 单张残局胜利被回滚、gid=2 叫分被回滚，双实测）。
    if row.state.phase == Phase::Done {
        return Ok(row);
    }
    if kicked {
        return Ok(row);
    }
    if !row.my_turn {
        return Err(DomainError::Validation("还没轮到你".into()));
    }
    let events_before = row.state.events.len();
    match action {
        "bid" => {
            let score = score.unwrap_or(0);
            row.state
                .bid(0, score)
                .map_err(|e| DomainError::Validation(e.to_string()))?;
        }
        "grab" => {
            let take = score.unwrap_or(0) != 0;
            row.state
                .grab(0, take)
                .map_err(|e| DomainError::Validation(e.to_string()))?;
        }
        "discard" => {
            row.state
                .discard(0, cards)
                .map_err(|e| DomainError::Validation(e.to_string()))?;
        }
        "double" => {
            let level = score.unwrap_or(0);
            row.state
                .double(0, level)
                .map_err(|e| DomainError::Validation(e.to_string()))?;
        }
        "play" => {
            row.state
                .play(0, cards)
                .map_err(|e| DomainError::Validation(e.to_string()))?;
        }
        _ => return Err(DomainError::Validation("未知动作".into())),
    }
    super::doudizhu_play_repo::run_ai_segments(tx, uid, &mut row).await?;
    // PvP：把本行新事件（玩家动作 + AI 段）扇出重放到同房另两行。
    super::doudizhu_play_repo::sync_mirror_tx(tx, &row, events_before).await?;
    Ok(row)
}
