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
use crate::economy_http::{spend_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::games;
use crate::http::require_auth;
use crate::state::AppState;

use super::casino::{award_kind, settle_tx};
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
    // 渔汛：周末窗口开着就抽活动塘；开着但本周做种未达标 → 明确拒绝并给差值
    let (ev_on, ev_ok, ev_hours, ev_need) =
        super::fishing_extra::event_state(&state, auth.id).await?;
    if ev_on && !ev_ok {
        return Err(DomainError::Validation(format!(
            "渔汛要求本周做种满 {ev_need} 小时（当前 {ev_hours} 小时）：\
             挂机做种攒门槛，周末来钓限定鱼"
        )));
    }
    let (pool_name, event) = if ev_on {
        ("fishing_event", true)
    } else {
        ("fishing", false)
    };
    let pool = load_pool(db, pool_name).await?;
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
    // 鱼竿加成：每级加一点起竿窗口（纯 sink 换手感，不改概率）
    let rod_lv: i32 = sqlx::query_scalar(
        "SELECT level FROM arcade_fishing_rods WHERE user_id = $1",
    )
    .bind(auth.id)
    .fetch_optional(db)
    .await
    .map_err(dberr)?
    .unwrap_or(1);
    let rod_bonus = eco_i64(&state, "fishing_rod_window_bonus_ms", 150)
        .await
        .clamp(0, 500)
        * i64::from(rod_lv - 1);
    let win_ms = (win_ms + rod_bonus).min(8000);
    let bite = rand::thread_rng().gen_range(min..=max);

    // 扣鱼饵与落下这一竿**必须同一事务**：分两次写的话，INSERT 失败就是
    // 「钱扣了、局没了」—— 玩家连起竿的机会都没有，只剩一笔冤枉流水。
    let mut tx = db.begin().await.map_err(dberr)?;
    let idem = idem_key("fish", auth.id, &body.idempotency_key);
    // 幂等：同一键重放不重复扣鱼饵，也不重开一竿
    if !matches!(
        spend_spark_tx(&mut tx, auth.id, body.bet, "game", &idem, "fishing", 0)
            .await?,
        SpendOutcome::Spent
    ) {
        let _ = tx.rollback().await;
        return Err(DomainError::Validation(
            "这一竿已受理，请勿重复提交".into(),
        ));
    }
    // 顺手清掉本用户超时未收线的残局：抛竿后不起竿，那一行就永远晾着，
    // 而它再也不会被任何人读到 —— 不清就是只涨不消的表。
    sqlx::query(
        "DELETE FROM arcade_fishing_rounds WHERE user_id = $1 \
           AND NOT resolved AND created_at < now() - interval '1 hour'",
    )
    .bind(auth.id)
    .execute(&mut *tx)
    .await
    .map_err(dberr)?;
    let round_id: i64 = sqlx::query_scalar(
        "INSERT INTO arcade_fishing_rounds \
             (user_id, bet, entry_index, bite_after_ms, window_ms, \
              event, idem) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.bet)
    .bind(draw.index as i32)
    .bind(bite)
    .bind(win_ms)
    .bind(event)
    .bind(&idem)
    .fetch_one(&mut *tx)
    .await
    .map_err(dberr)?;
    tx.commit().await.map_err(dberr)?;
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
    let row: Option<(i64, i32, i32, i32, i64, Option<String>, bool)> =
        sqlx::query_as(
            "SELECT bet, entry_index, bite_after_ms, window_ms, \
                    (EXTRACT(EPOCH FROM (now() - created_at)) * 1000)::bigint \
                      AS elapsed_ms, idem, event \
               FROM arcade_fishing_rounds \
              WHERE id = $1 AND user_id = $2 AND NOT resolved FOR UPDATE",
        )
        .bind(body.round_id)
        .bind(auth.id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(dberr)?;
    let Some((bet, idx, bite, win_ms, elapsed, idem, event)) = row else {
        let _ = tx.rollback().await;
        return Err(DomainError::Validation("这一竿已收线或不存在".into()));
    };
    let lo = i64::from(bite);
    let hi = lo + i64::from(win_ms);
    let hit = elapsed >= lo && elapsed <= hi;
    // 命中才需要奖池；未命中连池都不读，直接在同一事务里收线。
    // 池若在这几秒被站长改过：下标越界 → 回滚整笔（round 仍 NOT resolved，
    // 玩家重试时按新池结，不再出现「注已扣、奖结不出、round 永久晾死」）。
    let mut draw = None;
    if hit {
        let key = if event { "fishing_event" } else { "fishing" };
        let pool = load_pool(db, key).await?;
        let i = idx.max(0) as usize;
        let entry = match pool.entries.get(i).cloned() {
            Some(e) => e,
            None => {
                // 池在 cast→reel 之间被改：越界即回滚（round 保持可重试）。
                // rollback 也可能失败，但只丢一行未收线残局（1 小时后 cast
                // 侧的清理会兜底），不产生错账——warn 后按可重试错误返回。
                let _ = tx.rollback().await;
                return Err(DomainError::Validation(
                    "奖池已变更，请重试这一竿".into(),
                ));
            }
        };
        let rarity = super::casino::meta_rarity(&pool.meta, i);
        draw = Some((
            games::Draw {
                index: i,
                prize: entry,
            },
            rarity,
        ));
    }
    // 「收线 + 派彩」必须同一笔事务（2026-10 审计 P1）：旧实现先提交
    // resolved=true 再单独结算，中间失败=注已扣且 round 已死，无法重放。
    let win_idem = idem
        .clone()
        .map(|k| format!("game-fishing-win:{k}"))
        .unwrap_or_else(|| format!("game-fishing-win:{}", body.round_id));
    let (spark, value, fell_back) = if let Some((d, rarity)) = draw.as_ref() {
        settle_tx(&mut tx, auth.id, bet, d, "fishing", &win_idem, *rarity)
            .await?
    } else {
        (0i64, 0i64, None)
    };
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
    let d = draw.expect("hit 为真时 draw 必在").0;
    state.repo.audit(Some(auth.id), "game.fishing", None).await;
    Ok(ok(serde_json::json!({
        "won": value > 0,
        "prize": d.prize.label,
        "kind": award_kind(&d, fell_back),
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
