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
    let (bet, resolved, done_score, done_payout, idem): (
        i64,
        bool,
        i64,
        i64,
        String,
    ) = sqlx::query_as(
        "SELECT bet, resolved, COALESCE(score, 0), \
                    COALESCE(payout, 0), COALESCE(idem, '') \
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
    let values: Vec<i64> = hits.iter().map(|(_, v)| *v).collect();
    let (score, payout) = games::settle_score(&values);
    let win_idem = format!("game-mole-win:{idem}");
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    if payout > 0 {
        let paid =
            earn_spark_tx(&mut tx, auth.id, payout, "game", &win_idem).await?;
        // 上一笔已入账但没来得及标记 resolved（进程中断）时会重放：
        // 钱已经在账上，这里继续收尾即可，不能因为重放就跳过结算
        if matches!(paid, SpendOutcome::Replayed) {
            tracing::warn!(round_id = body.round_id, "mole payout replayed");
        }
    }
    let marked = sqlx::query(
        "UPDATE arcade_mole_rounds SET resolved = true, score = $2, \
                payout = $3, resolved_at = now() \
          WHERE id = $1 AND NOT resolved",
    )
    .bind(body.round_id)
    .bind(score)
    .bind(payout)
    .execute(&mut *tx)
    .await
    .map_err(dberr)?;
    if marked.rows_affected() == 0 {
        // 并发双击 finish：让对方那一笔赢，本笔回滚
        let _ = tx.rollback().await;
        return Err(DomainError::Validation("这一局已结算，请刷新战绩".into()));
    }
    // 图鉴/稀有播报的事实源：每只中奖地鼠一行（炸弹与空档不记）
    record_tiers(&mut tx, auth.id, body.round_id, &hits).await?;
    tx.commit().await.map_err(dberr)?;
    state
        .repo
        .audit(Some(auth.id), "game.mole.finish", None)
        .await;
    Ok(ok(finish_view(body.round_id, bet, score, payout, &hits)))
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
