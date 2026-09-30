//! 钓鱼（计时小游戏）：抛竿 → 咬钩 → 窗口内起竿。
//!
//! 服务端权威：**结果在抛竿那一刻定死**（抽档 + 掷咬钩时刻落库），起竿只判时机。
//! 这样「起竿」纯粹是反应，规则一眼可解释；也堵死「先看结果再决定起不起竿」。
//!
//! 时序参数走设置键（缺省即可玩，站长可调）：`fishing_bite_min_ms` /
//! `fishing_bite_max_ms` / `fishing_window_ms`。

use actix_web::{post, web, HttpRequest, HttpResponse};
use rand::Rng;
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::{spend_spark, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::games;
use crate::http::require_auth;
use crate::state::AppState;

use super::casino::{award_kind, settle};
use super::helpers::{check_bet, check_rate, eco_i64, idem_key};
use super::pool::{dberr, load_pool};

#[derive(Deserialize)]
struct CastReq {
    bet: i64,
    #[serde(default)]
    idempotency_key: Option<String>,
}

/// 抛竿：扣鱼饵、定下这一竿的档位与咬钩时刻，返回判时机的两个数。
#[post("/games/fishing/cast")]
pub(super) async fn fishing_cast(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CastReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;
    let pool = load_pool(db, "fishing").await?;
    check_bet(&state, body.bet)
        .await
        .map_err(DomainError::Validation)?;
    if body.bet < pool.ticket {
        return Err(DomainError::Validation(format!(
            "最低鱼饵为 {} 魔力",
            pool.ticket
        )));
    }
    check_rate(&state, &state.redis, auth.id).await?;

    // 结果在抛竿定死：抽档 + 掷咬钩时刻
    let draw = games::draw_entry(&pool.entries)
        .ok_or_else(|| DomainError::Validation("钓鱼奖池不可抽样".into()))?;
    let min = eco_i64(&state, "fishing_bite_min_ms", 1500)
        .await
        .clamp(300, 15000);
    let max = eco_i64(&state, "fishing_bite_max_ms", 4500)
        .await
        .clamp(min, 20000);
    let win_ms = eco_i64(&state, "fishing_window_ms", 1500)
        .await
        .clamp(400, 5000);
    let bite = rand::thread_rng().gen_range(min..=max);

    let idem = idem_key("fish", auth.id, &body.idempotency_key);
    // 幂等：同一键重放不重复扣鱼饵，也不重开一竿
    if !matches!(
        spend_spark(db, auth.id, body.bet, "game", &idem, "fishing", 0).await?,
        SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "这一竿已受理，请勿重复提交".into(),
        ));
    }
    let round_id: i64 = sqlx::query_scalar(
        "INSERT INTO arcade_fishing_rounds \
             (user_id, bet, entry_index, bite_after_ms, window_ms, idem) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.bet)
    .bind(draw.index as i32)
    .bind(bite)
    .bind(win_ms)
    .bind(&idem)
    .fetch_one(db)
    .await
    .map_err(dberr)?;
    Ok(ok(serde_json::json!({
        "round_id": round_id,
        "bet": body.bet,
        "bite_after_ms": bite,
        "window_ms": win_ms,
    })))
}

#[derive(Deserialize)]
struct ReelReq {
    round_id: i64,
}

/// 起竿：按「咬钩后窗口」判时机。命中则按 cast 定下的档位结算；否则跑空。
#[post("/games/fishing/reel")]
pub(super) async fn fishing_reel(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ReelReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;
    // 锁行 + 取「已过去多久」（时机由服务端算，不信客户端报的毫秒数）
    let mut tx = db.begin().await.map_err(dberr)?;
    let row: Option<(i64, i32, i32, i32, i64, Option<String>)> =
        sqlx::query_as(
            "SELECT bet, entry_index, bite_after_ms, window_ms, \
                    (EXTRACT(EPOCH FROM (now() - created_at)) * 1000)::bigint \
                      AS elapsed_ms, idem \
               FROM arcade_fishing_rounds \
              WHERE id = $1 AND user_id = $2 AND NOT resolved FOR UPDATE",
        )
        .bind(body.round_id)
        .bind(auth.id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(dberr)?;
    let Some((bet, idx, bite, win_ms, elapsed, idem)) = row else {
        let _ = tx.rollback().await;
        return Err(DomainError::Validation(
            "这一竿已收线或不存在".into(),
        ));
    };
    let lo = i64::from(bite);
    let hi = lo + i64::from(win_ms);
    let hit = elapsed >= lo && elapsed <= hi;
    sqlx::query(
        "UPDATE arcade_fishing_rounds \
            SET resolved = true, won = $2, resolved_at = now() WHERE id = $1",
    )
    .bind(body.round_id)
    .bind(hit)
    .execute(&mut *tx)
    .await
    .map_err(dberr)?;
    tx.commit().await.map_err(dberr)?;

    if !hit {
        state
            .repo
            .audit(Some(auth.id), "game.fishing.miss", None)
            .await;
        return Ok(ok(serde_json::json!({
            "won": false, "payout": 0, "value": 0, "net": -bet,
            "bet": bet, "elapsed_ms": elapsed,
            "bite_after_ms": bite, "window_ms": win_ms,
        })));
    }
    // 命中：按 cast 定下的档位结算（池若在这几秒内被改过，下标越界就安全失败）
    let pool = load_pool(db, "fishing").await?;
    let i = idx.max(0) as usize;
    let entry = pool
        .entries
        .get(i)
        .cloned()
        .ok_or_else(|| DomainError::Validation("奖池已变更，这一竿无法结算".into()))?;
    let draw = games::Draw { index: i, prize: entry };
    let win_idem = format!(
        "game-fishing-win:{}",
        idem.unwrap_or_else(|| body.round_id.to_string())
    );
    let (spark, value, fell_back) =
        settle(&state, auth.id, bet, &draw, "fishing", &win_idem).await?;
    state.repo.audit(Some(auth.id), "game.fishing", None).await;
    Ok(ok(serde_json::json!({
        "won": value > 0,
        "prize": draw.prize.label,
        "kind": award_kind(&draw, fell_back),
        "fell_back": fell_back,
        "payout": spark,
        "value": value,
        "net": value - bet,
        "bet": bet,
        "elapsed_ms": elapsed,
        "bite_after_ms": bite,
        "window_ms": win_ms,
    })))
}
