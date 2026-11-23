//! 打地鼠两个口：开局（扣票 + 定排程）/ 敲击（当场判时机）。
//! 结算拆在 mole_settle.rs（300 行门禁）。
//!
//! 与钓鱼同一条哲学：**结果在开局定死，敲击只判时机**。差别是钓鱼只判
//! 一次起竿，这里一局要判 14 次；因此命中逐条落表（主键防重复），
//! finish 只汇总服务端记录——客户端算出来的分数一律不作数。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::{spend_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::games;
use crate::http::require_auth;
use crate::state::AppState;

use super::helpers::{check_minclass, check_rate, eco_i64, idem_key};
use super::pool::{dberr, load_pool};

type Db = web::Data<std::sync::Arc<AppState>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartReq {
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HitReq {
    round_id: i64,
    seq: i32,
}

#[post("/games/mole/start")]
pub(super) async fn mole_start(
    req: HttpRequest,
    state: Db,
    body: crate::http::OptJson<StartReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "mole", auth.class_id).await?;
    let pool = load_pool(&state.repo.db, "mole").await?;
    // 读侧再闸一次：池子在后台被改宽时，这里 fail-closed
    games::validate_mole(&pool.entries, pool.ticket)
        .map_err(|m| DomainError::Validation(m.into()))?;
    let max_bet = eco_i64(&state, "games_max_bet", games::MAX_BET).await;
    if pool.ticket > max_bet {
        return Err(DomainError::Validation(
            "票价超过单次下注上限，当前配置下无法开局".into(),
        ));
    }
    check_rate(&state, &state.redis, auth.id).await?;
    let penalty =
        eco_i64(&state, "mole_bomb_penalty", games::DEFAULT_BOMB_PENALTY)
            .await
            .clamp(0, 100);
    let mut rng = rand::thread_rng();
    let rarities: Vec<i16> = pool.meta.iter().map(|(r, _)| *r).collect();
    let spawns = games::build_schedule(
        &pool.entries,
        pool.ticket,
        penalty,
        &rarities,
        &mut rng,
    );
    let client_idem = body.and_then(|b| b.idempotency_key.clone());
    let idem = idem_key("mole", auth.id, &client_idem);

    let ticket = pool.ticket;
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    let outcome =
        spend_spark_tx(&mut tx, auth.id, ticket, "game", &idem, "mole", 0)
            .await?;
    let replay: Option<(i64, serde_json::Value)> = match outcome {
        SpendOutcome::Spent => None,
        SpendOutcome::Replayed => sqlx::query_as(
            "SELECT id, schedule FROM arcade_mole_rounds \
              WHERE user_id = $1 AND idem = $2 AND NOT resolved",
        )
        .bind(auth.id)
        .bind(&idem)
        .fetch_optional(&mut *tx)
        .await
        .map_err(dberr)?,
    };

    let (round_id, spawns) = match replay {
        Some((round_id, schedule)) => {
            let _ = tx.rollback().await;
            let spawns: Vec<games::Spawn> = serde_json::from_value(schedule)
                .map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;
            (round_id, spawns)
        }
        None => {
            if !matches!(outcome, SpendOutcome::Spent) {
                let _ = tx.rollback().await;
                return Err(DomainError::Validation(
                    "该局已受理，请勿重复提交".into(),
                ));
            }
            // 只清自己的过期未结算：排程里没有玩家余额，删了也不动账
            sqlx::query(
                "DELETE FROM arcade_mole_rounds \
                  WHERE user_id = $1 AND NOT resolved \
                    AND created_at < now() - interval '1 hour'",
            )
            .bind(auth.id)
            .execute(&mut *tx)
            .await
            .map_err(dberr)?;
            let round_id: i64 = sqlx::query_scalar(
                "INSERT INTO arcade_mole_rounds (user_id, bet, schedule, idem) \
                 VALUES ($1, $2, $3, $4) RETURNING id",
            )
            .bind(auth.id)
            .bind(ticket)
            .bind(serde_json::to_value(&spawns).unwrap_or_default())
            .bind(&idem)
            .fetch_one(&mut *tx)
            .await
            .map_err(dberr)?;
            tx.commit().await.map_err(dberr)?;
            (round_id, spawns)
        }
    };
    state
        .repo
        .audit(Some(auth.id), "game.mole.start", None)
        .await;
    Ok(ok(serde_json::json!({
        "round_id": round_id,
        "ticket": ticket,
        "round_ms": games::ROUND_MS,
        "spawns": spawns,
        "hit_early_ms": games::HIT_EARLY_MS,
        "hit_late_ms": games::HIT_LATE_MS,
    })))
}

#[post("/games/mole/hit")]
pub(super) async fn mole_hit(
    req: HttpRequest,
    state: Db,
    body: web::Json<HitReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;
    let (schedule, resolved, elapsed): (serde_json::Value, bool, i64) =
        sqlx::query_as(
            "SELECT schedule, resolved, \
                    (EXTRACT(EPOCH FROM (now() - created_at)) * 1000)::bigint \
               FROM arcade_mole_rounds WHERE id = $1 AND user_id = $2",
        )
        .bind(body.round_id)
        .bind(auth.id)
        .fetch_optional(db)
        .await
        .map_err(dberr)?
        .ok_or_else(|| {
            DomainError::Validation("这一局不存在或不属于你".into())
        })?;
    if resolved {
        return Err(DomainError::Validation("这一局已结算".into()));
    }
    let spawns: Vec<games::Spawn> = serde_json::from_value(schedule)
        .map_err(|e| DomainError::Internal(anyhow::anyhow!(e)))?;
    let spawn = spawns.iter().find(|s| s.seq == body.seq).ok_or_else(|| {
        DomainError::Validation("这一锤不在本局排程里".into())
    })?;
    // 放空不罚：窗口外回 hit=false，前端只演一条空挥动效
    if !games::in_window(spawn, elapsed) {
        return Ok(ok(serde_json::json!({
            "hit": false, "value": 0, "label": spawn.label,
        })));
    }
    let inserted = sqlx::query(
        "INSERT INTO arcade_mole_hits (round_id, seq, value) \
         VALUES ($1, $2, $3) ON CONFLICT (round_id, seq) DO NOTHING",
    )
    .bind(body.round_id)
    .bind(body.seq)
    .bind(spawn.value)
    .execute(db)
    .await
    .map_err(dberr)?;
    Ok(ok(serde_json::json!({
        "hit": true,
        "dup": inserted.rows_affected() == 0,
        "value": spawn.value,
        "label": spawn.label,
        "idx": spawn.idx,
    })))
}
