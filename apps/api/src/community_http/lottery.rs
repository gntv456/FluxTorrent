//! M15 论坛抽奖（参与/开奖）。
//! 从 community_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::*;
use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
struct LotteryJoinReq {
    topic_id: i64,
}

/// 参与抽奖：付票价（0=免费）换一个名额。约束：
/// · 仅 open 且未到 draw_at（到点等 worker 开奖，不接受「补票」）；
/// · 楼主不能参与自己的抽奖（既当庄又下注必起纠纷）；
/// · 一人一次（PK 幂等）；票价与占位原子（对齐 fun_vote/jgg 的 spend-then-insert 回滚纪律，此处反过来 insert-then-spend 失败删占位）。
#[post("/forums/lottery/join")]
async fn lottery_join(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<LotteryJoinReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let l: Option<(i64, i64, String, chrono::DateTime<chrono::Utc>, i64)> = sqlx::query_as(
        "SELECT tl.ticket_spark::bigint, t.user_id, tl.status, tl.draw_at, t.forum_id \
         FROM topic_lotteries tl JOIN topics t ON t.id = tl.topic_id WHERE tl.topic_id = $1",
    )
    .bind(body.topic_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((ticket, op, status, draw_at, fid)) = l else {
        return Err(DomainError::NotFound(body.topic_id));
    };
    let perm =
        forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_read {
        return Err(DomainError::Forbidden);
    }
    if status != "open" {
        return Err(DomainError::Validation("抽奖不在进行中".into()));
    }
    if chrono::Utc::now() >= draw_at {
        return Err(DomainError::Validation("已到开奖时间，等待开奖".into()));
    }
    if op == auth.id {
        return Err(DomainError::Validation("楼主不能参与自己的抽奖".into()));
    }
    let entered = sqlx::query(
        "INSERT INTO lottery_entries (topic_id, user_id) VALUES ($1, \
         $2) ON CONFLICT DO NOTHING",
    )
    .bind(body.topic_id)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if entered == 0 {
        return Err(DomainError::Validation("已经参与过了".into()));
    }
    // 票价（免费则跳过）：扣费失败回滚占位（对齐 fun_vote 纪律）
    if ticket > 0 {
        let idem =
            format!("forum-lottery-ticket:{}:{}", auth.id, body.topic_id);
        if let Err(e) = crate::economy_http::spend_spark(
            &state.repo.db,
            auth.id,
            ticket,
            "forum_lottery",
            &idem,
            "forum_lottery",
            body.topic_id,
        )
        .await
        {
            let _ = sqlx::query(
                "DELETE FROM lottery_entries WHERE \
             topic_id = $1 AND user_id = $2",
            )
            .bind(body.topic_id)
            .bind(auth.id)
            .execute(&state.repo.db)
            .await;
            return Err(e);
        }
    }
    state
        .repo
        .audit(Some(auth.id), "forum.lottery_join", Some(body.topic_id))
        .await;
    Ok(ok(
        serde_json::json!({ "topic_id": body.topic_id, "ticket": ticket }),
    ))
}

/// 开奖核心（手动入口与 worker 共用）：CAS open→drawn 后随机抽 winners 名，
/// 每人发 prize_per_winner（幂等键 `forum-lottery-win:{tid}:{uid}`）。
/// 参与人数不足名额时全中奖（钱不留在池里）；零参与则奖金池退回楼主。
pub async fn lottery_draw_core(
    db: &sqlx::PgPool,
    topic_id: i64,
) -> DomainResult<serde_json::Value> {
    let l: Option<(i32, i64, i64, String)> = sqlx::query_as(
        "SELECT winners, prize_per_winner, ticket_spark::bigint, \
         status FROM topic_lotteries WHERE topic_id = $1",
    )
    .bind(topic_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((winners, prize, _ticket, status)) = l else {
        return Err(DomainError::NotFound(topic_id));
    };
    if status != "open" {
        return Err(DomainError::Validation("抽奖不在进行中".into()));
    }
    let n = sqlx::query(
        "UPDATE topic_lotteries SET status = 'drawn' WHERE topic_id = \
         $1 AND status = 'open'",
    )
    .bind(topic_id)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::LedgerConflict);
    }
    // 参与者全表捞出来在应用层抽（数量级 ≤ 数百，RANDOM() 洗牌即可）
    let mut entries: Vec<i64> = sqlx::query_scalar(
        "SELECT user_id FROM lottery_entries WHERE topic_id = $1",
    )
    .bind(topic_id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if entries.is_empty() {
        // 无人参与：奖金池退回楼主
        let op: i64 =
            sqlx::query_scalar("SELECT user_id FROM topics WHERE id = $1")
                .bind(topic_id)
                .fetch_one(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        let refund = winners as i64 * prize;
        if refund > 0 {
            let _ = earn_spark(
                db,
                op,
                refund,
                "forum_lottery_refund",
                &format!("forum-lottery-refund:{topic_id}"),
            )
            .await;
        }
        return Ok(
            serde_json::json!({ "topic_id": topic_id, "winners": [], "refunded": refund }),
        );
    }
    use rand::seq::SliceRandom;
    entries.shuffle(&mut rand::thread_rng());
    let take = (winners as usize).min(entries.len());
    let picked: Vec<i64> = entries.into_iter().take(take).collect();
    for uid in &picked {
        let _ = sqlx::query(
            "UPDATE lottery_entries SET won = TRUE WHERE topic_id \
             = $1 AND user_id = $2",
        )
        .bind(topic_id)
        .bind(uid)
        .execute(db)
        .await;
        if prize > 0 {
            let _ = earn_spark(
                db,
                *uid,
                prize,
                "forum_lottery",
                &format!("forum-lottery-win:{topic_id}:{uid}"),
            )
            .await;
            notify_user(
                db,
                *uid,
                "抽奖中奖",
                &format!("您在 [/forums/topic/{topic_id}] 的抽奖中中奖，获得 {prize} 魔力！"),
            )
            .await;
        }
    }
    Ok(
        serde_json::json!({ "topic_id": topic_id, "winners": picked, "prize": prize }),
    )
}

#[derive(Deserialize)]
struct LotteryDrawReq {
    topic_id: i64,
}

/// 楼主手动开奖（提前开或到点 worker 没来得及时的兜底）。版主亦可（治理口径，同 poll_close）。
#[post("/forums/lottery/draw")]
async fn lottery_draw(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<LotteryDrawReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let l: Option<(i64, i64)> = sqlx::query_as(
        "SELECT t.user_id, t.forum_id FROM topic_lotteries tl JOIN topics t ON t.id = tl.topic_id \
         WHERE tl.topic_id = $1",
    )
    .bind(body.topic_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((op, fid)) = l else {
        return Err(DomainError::NotFound(body.topic_id));
    };
    let perm =
        forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if auth.id != op && !perm.can_mod {
        return Err(DomainError::Forbidden);
    }
    let out = lottery_draw_core(&state.repo.db, body.topic_id).await?;
    state
        .repo
        .audit(Some(auth.id), "forum.lottery_draw", Some(body.topic_id))
        .await;
    Ok(ok(out))
}

// ---- 论坛打赏（0127）：楼层打赏 = spend(打赏人) + earn(作者) 同额对冲，不抽税 ----
