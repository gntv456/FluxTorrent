//! 删主题（DELETE /forums/topics/{id}）从 post_edit.rs 按域拆出。
//! post_edit.rs 加 utoipa 注解后贴近 300 行门禁，按域拆出。
//!
//! 2026-10-16：SQL 已下沉到 repo::community::{topic_delete, follows}
//! （H 层不 import sqlx，FRAME §2）。本文件只留权限判定、钱的回收
//! 编排（earn_spark 的幂等键在 H 层）与 JSON 组装。

use super::forum_access;
use crate::dto::ok;
use crate::economy_http::{clawback_spark, earn_spark};
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::repo::community::follows as repo_follows;
use crate::repo::community::forum_reward;
use crate::repo::community::topic_delete as repo_td;
use crate::state::AppState;
use actix_web::{delete, web, HttpRequest, HttpResponse};

/// 删主题：仅版主/postmanage；收回发帖 +2 火花（KPS("-", starttopic_bonus) 口径）
#[utoipa::path(
    delete,
    path = "/api/v1/forums/topics/{id}",
    tag = "社区/论坛",
    description = "删主题（仅版主；级联删回帖）。",
    params(
        ("id" = i64, description = "主题 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 403, description = "无删帖权（仅版主）（code 2003）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[delete("/forums/topics/{id}")]
pub(crate) async fn topic_delete(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    let Some(fid) = repo_td::topic_forum_id(&state.repo, tid).await? else {
        return Err(DomainError::NotFound(tid));
    };
    let perm =
        forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_mod {
        return Err(DomainError::Forbidden);
    }
    let op = repo_td::topic_owner(&state.repo, tid).await?;
    // 悬赏未决退回（0124）：open 状态的悬赏在删主题时退还楼主（已 awarded 的不动——钱已归答主）。
    // 幂等键与发放错开：`-refund` 后缀，退回与发放都各只发生一次。
    if let Some(op_id) = op {
        let bounty_open = repo_td::open_bounty(&state.repo, tid).await?;
        if let Some(refund) = bounty_open {
            // P-364：钱到账才落「已退」标记。旧版 `let _ = earn_spark` 吞掉失败，
            // 紧接着无条件 mark_bounty_refunded ⇒ 楼主的悬赏被记成「已退回」，
            // 而魔力根本没进他账户，且标记之后再也重试不了（open_bounty 只认
            // open 状态）。现在：失败就出声并**跳过标记**，悬赏留在 open 态可重试，
            // 幂等键 `forum-bounty-refund:{tid}` 保证真退只退一次。
            if let Err(e) = earn_spark(
                &state.repo.db,
                op_id,
                refund,
                "forum_bounty_refund",
                &format!("forum-bounty-refund:{tid}"),
            )
            .await
            {
                tracing::warn!(
                    err = %e,
                    topic = tid,
                    owner = op_id,
                    "bounty refund failed: left open for retry"
                );
            } else {
                let _ = repo_td::mark_bounty_refunded(&state.repo, tid).await;
            }
        }
        // 抽奖未决退回（0126）：open 状态的抽奖在删主题时奖金池退还楼主（drawn 的不动——钱已归中奖人）。
        // 票价不退（参与者享受了参与过程；与删悬赏帖不追回已发赏金同一不对称口径）。
        let lot_open = repo_td::open_lottery_pool(&state.repo, tid).await?;
        if let Some(refund) = lot_open {
            // P-364 同式：奖金池到账才落「已取消」，失败就出声并留 open 可重试
            if let Err(e) = earn_spark(
                &state.repo.db,
                op_id,
                refund,
                "forum_lottery_refund",
                &format!("forum-lottery-refund:{tid}"),
            )
            .await
            {
                tracing::warn!(
                    err = %e,
                    topic = tid,
                    owner = op_id,
                    "lottery refund failed: left open for retry"
                );
            } else {
                let _ = repo_td::mark_lottery_cancelled(&state.repo, tid).await;
            }
        }
    }
    if let Some(op_id) = op {
        // P-415：收回走地板原语（旧写法把 -2 送进 earn_spark：发放原语不防负，
        // `spark_balance` 也没有 CHECK，零余额楼主会被直接写成 -2，且 `let _ =`
        // 把失败吞干净）。能收多少收多少、流水与余额同步、失败出声。
        // P-456：收回额回查当年那笔发放（键 forum-topic:{uid}:{tid}），
        // 不再写死 2——面板 starttopic 改成 5 之后删老帖会多扣 3。
        // 查不到流水＝当时没发过；clawback 原语对 <=0 判 Err，必须跳过，
        // 否则刷「收回失败」假告警。
        let owed = match forum_reward::granted_by_idem(
            &state.repo,
            &forum_reward::topic_idem(op_id, tid),
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    err = %e,
                    topic = tid,
                    "删主题前回查发帖发放流水失败：本笔不收回"
                );
                0
            }
        };
        if owed > 0 {
            if let Err(e) = clawback_spark(
                &state.repo.db,
                op_id,
                owed,
                "forum-topic-del",
                &format!("forum-topic-del:{tid}"),
            )
            .await
            {
                tracing::warn!(
                    err = %e,
                    topic = tid,
                    user = op_id,
                    "删主题收回发帖火花失败"
                );
            }
        }
    }
    // 级联回收回帖奖励：主题删除会把全部回帖 CASCADE 掉，逐笔按当年发放额
    // 收回（P-456：reply_reward_debt 交的是流水汇总，不再乘常数）。
    // 审计修复（P1 双重扣分）：楼主首帖也存于 posts（topic_create 落 posts 行），
    // 旧版把它计为「回帖」再 -1，楼主实扣 -3（发帖 -2 + 首帖按回帖 -1）。
    // 排除楼主首帖，楼主只按发帖口径 -2。
    let replies =
        repo_td::reply_reward_debt(&state.repo, tid, op.unwrap_or(0)).await;
    for (uid, n) in replies {
        if n <= 0 {
            continue;
        }
        // P-415：级联收回同样走地板原语；逐人夹零，不会把某个回帖作者写成负账
        if let Err(e) = clawback_spark(
            &state.repo.db,
            uid,
            n as i64,
            "forum-topic-del-reply",
            &format!("forum-topic-del-reply:{tid}:{uid}"),
        )
        .await
        {
            tracing::warn!(
                err = %e,
                topic = tid,
                user = uid,
                "删主题级联收回回帖火花失败"
            );
        }
    }
    repo_td::delete_topic(&state.repo, tid).await?;
    // 关注本主题的行：target_id 无外键（指向 3 张表），必须应用层清理，否则留下悬空关注
    let _ = repo_follows::drop_topic_follows(&state.repo, tid).await;
    state
        .repo
        .audit(Some(auth.id), "forum.topic_delete", Some(tid))
        .await;
    Ok(ok(serde_json::json!({ "deleted": tid })))
}
