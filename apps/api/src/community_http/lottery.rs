//! M15 论坛抽奖（参与/开奖）。
//! 从 community_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::*;
use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::repo::lottery_fair as repo_fair;
use crate::state::AppState;

#[derive(Deserialize, utoipa::ToSchema)]
struct LotteryJoinReq {
    topic_id: i64,
}

/// 参与抽奖：付票价（0=免费）换一个名额。约束：
/// · 仅 open 且未到 draw_at（到点等 worker 开奖，不接受「补票」）；
/// · 楼主不能参与自己的抽奖（既当庄又下注必起纠纷）；
/// · 一人一次（PK 幂等）；占位、票据链、票价同事务（P-396）——扣不掉
///   就不会白占名额，链断了就不会假装开过奖。
#[utoipa::path(
    post,
    path = "/api/v1/forums/lottery/join",
    tag = "社区/论坛",
    description = "参与论坛抽奖（占位、票据链、票价同事务）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[post("/forums/lottery/join")]
pub(crate) async fn lottery_join(
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
    // commit-reveal（0379）：公平路径要给这张票编号并接上哈希链。
    // P-396：占位、票据链、票价三段旧写法各自 autocommit——先落占位行，再让
    // `issue_ticket` 自己开事务插一条 `ON CONFLICT DO NOTHING`（必冲突 ⇒
    // 哈希永不落库 ⇒ 开奖时 `picked` 为空、奖池既不派彩也不再退回），扣费又
    // 只判 Err（放过 Replayed）。现在三段并进本事务，任何一段失败整体回滚。
    let fair_seed = if repo_fair::is_fair(&state.repo, body.topic_id).await? {
        repo_fair::seed_of(&state.repo, body.topic_id).await?
    } else {
        None
    };
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 同场售票串行：链尾推进必须有序，否则两条从 genesis 起链的分叉无法
    // 复算。xact 级锁随 commit/rollback 自动释放（同 repo/auth.rs 用户量闸）。
    const LOTTERY_JOIN_LOCK: i64 = 0x666c_746c_795f_0000;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(LOTTERY_JOIN_LOCK | body.topic_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let row_ts: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "INSERT INTO lottery_entries (topic_id, user_id) VALUES ($1, $2) \
         ON CONFLICT (topic_id, user_id) DO NOTHING RETURNING created_at",
    )
    .bind(body.topic_id)
    .bind(auth.id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(row_ts) = row_ts else {
        return Err(DomainError::Validation("已经参与过了".into()));
    };
    let ticket_hash = match fair_seed {
        Some(sd) => Some(
            repo_fair::issue_ticket_tx(&mut tx, body.topic_id, auth.id, &sd)
                .await?,
        ),
        None => None,
    };
    // 票价（免费则跳过）：键锚在**这一行**（created_at 微秒）上 ⇒ 「占位行
    // 消失而幂等键还在」这一族白拿口从结构上不存在（P-380 同式）；
    // Replayed 一律 fail-close，不放行成没扣钱的参与。
    if ticket > 0 {
        let idem = format!(
            "forum-lottery-ticket:{}:{}:{}",
            auth.id,
            body.topic_id,
            row_ts.timestamp_micros()
        );
        if !matches!(
            crate::economy_http::spend_spark_tx(
                &mut tx,
                auth.id,
                ticket,
                "forum_lottery",
                &idem,
                "forum_lottery",
                body.topic_id,
            )
            .await?,
            crate::economy_http::SpendOutcome::Spent
        ) {
            return Err(DomainError::Validation(
                "这张票的票价已受理，未重复扣款".into(),
            ));
        }
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "forum.lottery_join", Some(body.topic_id))
        .await;
    Ok(ok(serde_json::json!({
        "topic_id": body.topic_id,
        "ticket": ticket,
        "ticket_hash": ticket_hash,
    })))
}

#[derive(Deserialize, utoipa::ToSchema)]
struct LotteryDrawReq {
    topic_id: i64,
}

/// 楼主手动开奖（提前开或到点 worker 没来得及时的兜底）。版主亦可（治理口径，同 poll_close）。
#[utoipa::path(
    post,
    path = "/api/v1/forums/lottery/draw",
    tag = "社区/论坛",
    description = "开奖（发起人；事务内防重放）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 403, description = "非发起人（code 2003）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[post("/forums/lottery/draw")]
pub(crate) async fn lottery_draw(
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
    let out = lottery_draw_core(&state.repo, body.topic_id).await?;
    state
        .repo
        .audit(Some(auth.id), "forum.lottery_draw", Some(body.topic_id))
        .await;
    Ok(ok(out))
}

// ---- 论坛打赏（0127）：楼层打赏 = spend(打赏人) + earn(作者) 同额对冲，不抽税 ----
