//! 打地鼠结算：finish 幂等读回、逐档落图鉴与稀有播报、结果视图。
//!
//! 从 mole.rs 拆出（300 行门禁）：开局与敲击只认排程，结算是另一条
//! 资金路径（入账 + 标记 resolved + 图鉴行必须同一笔事务）。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::{earn_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::games;
use crate::http::require_auth;
use crate::state::AppState;

use super::pool::dberr;
use crate::games::mole_repo::settle_round_tx;

type Db = web::Data<std::sync::Arc<AppState>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EndReq {
    round_id: i64,
}

#[post("/games/mole/finish")]
pub(super) async fn mole_finish(
    req: HttpRequest,
    state: Db,
    body: web::Json<EndReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;
    let (bet, resolved, done_score, done_payout, idem, schedule, elapsed): (
        i64,
        bool,
        i64,
        i64,
        String,
        serde_json::Value,
        i64,
    ) = sqlx::query_as(
        "SELECT bet, resolved, COALESCE(score, 0), \
                    COALESCE(payout, 0), COALESCE(idem, ''), schedule, \
                    (EXTRACT(EPOCH FROM (now() - created_at)) * 1000)::bigint \
               FROM arcade_mole_rounds WHERE id = $1 AND user_id = $2",
    )
    .bind(body.round_id)
    .bind(auth.id)
    .fetch_optional(db)
    .await
    .map_err(dberr)?
    .ok_or_else(|| DomainError::Validation("这一局不存在或不属于你".into()))?;
    // 结算幂等：已结算就原样回读，不重复入账
    if resolved {
        let hits: Vec<(i32, i64)> = sqlx::query_as(
            "SELECT seq, value FROM arcade_mole_hits WHERE round_id = $1",
        )
        .bind(body.round_id)
        .fetch_all(db)
        .await
        .map_err(dberr)?;
        return Ok(ok(finish_view(
            body.round_id,
            bet,
            done_score,
            done_payout,
            &hits,
        )));
    }
    let hits: Vec<(i32, i64)> = sqlx::query_as(
        "SELECT seq, value FROM arcade_mole_hits WHERE round_id = $1",
    )
    .bind(body.round_id)
    .fetch_all(db)
    .await
    .map_err(dberr)?;
    // 排程没走完不许结算（理由见 games::schedule_done）：切标签页 / 卸载
    // 触发的提前 finish 在这里被挡下，这一局留在未结算态，等走完再结。
    // 排程损坏的局由整局时长闸放行（见 games::settled_or_over）。
    let spawns: Vec<games::Spawn> =
        serde_json::from_value(schedule).unwrap_or_default();
    if !games::settled_or_over(&spawns, elapsed) {
        return Err(DomainError::Validation(
            "这一局还没结束，稍后再结算".into(),
        ));
    }
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    let (score, payout) =
        settle_round_tx(&mut tx, auth.id, body.round_id, &idem, &hits).await?;
    tx.commit().await.map_err(dberr)?;
    state
        .repo
        .audit(Some(auth.id), "game.mole.finish", None)
        .await;
    Ok(ok(finish_view(body.round_id, bet, score, payout, &hits)))
}

fn finish_view(
    round_id: i64,
    bet: i64,
    score: i64,
    payout: i64,
    hits: &[(i32, i64)],
) -> serde_json::Value {
    let bombs = hits.iter().filter(|(_, v)| *v < 0).count() as i64;
    // 「命中」只数地鼠（含空手档），炸弹单列；hits.len() 会把两者混在一起
    let hit_moles = hits.len() as i64 - bombs;
    let best = hits
        .iter()
        .map(|(_, v)| *v)
        .filter(|v| *v >= 0)
        .max()
        .unwrap_or(0);
    serde_json::json!({
        "round_id": round_id,
        "score": score,
        "payout": payout,
        "net": payout - bet,
        "ticket": bet,
        "hits": hit_moles,
        "bombs": bombs,
        "best": best,
        "resolved": true,
    })
}
