//! 删主题（DELETE /forums/topics/{id}）从 post_edit.rs 按域拆出。
//! post_edit.rs 加 utoipa 注解后贴近 300 行门禁，按域拆出。
//!
//! 2026-10-16：SQL 已下沉到 repo::community::{topic_delete, follows}
//! （H 层不 import sqlx，FRAME §2）。本文件只留权限判定、钱的回收
//! 编排（earn_spark 的幂等键在 H 层）与 JSON 组装。

use super::forum_access;
use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::repo::community::follows as repo_follows;
use crate::repo::community::topic_delete as repo_td;
use crate::state::AppState;
use actix_web::{delete, web, HttpRequest, HttpResponse};

/// 删主题：仅版主/postmanage；收回发帖 +2 火花（KPS("-", starttopic_bonus) 口径）
#[utoipa::path(
    delete,
    path = "/api/v1/forums/topics/{id}",
    tag = "社区/论坛",
    description = "删主题（作者或版主；级联删回帖）。",
    params(
        ("id" = i64, description = "主题 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 403, description = "非作者且无删帖权（code 2003）"),
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
            let _ = earn_spark(
                &state.repo.db,
                op_id,
                refund,
                "forum_bounty_refund",
                &format!("forum-bounty-refund:{tid}"),
            )
            .await;
            let _ = repo_td::mark_bounty_refunded(&state.repo, tid).await;
        }
        // 抽奖未决退回（0126）：open 状态的抽奖在删主题时奖金池退还楼主（drawn 的不动——钱已归中奖人）。
        // 票价不退（参与者享受了参与过程；与删悬赏帖不追回已发赏金同一不对称口径）。
        let lot_open = repo_td::open_lottery_pool(&state.repo, tid).await?;
        if let Some(refund) = lot_open {
            let _ = earn_spark(
                &state.repo.db,
                op_id,
                refund,
                "forum_lottery_refund",
                &format!("forum-lottery-refund:{tid}"),
            )
            .await;
            let _ = repo_td::mark_lottery_cancelled(&state.repo, tid).await;
        }
    }
    if let Some(op_id) = op {
        let _ = earn_spark(
            &state.repo.db,
            op_id,
            -2,
            "forum-topic-del",
            &format!("forum-topic-del:{tid}"),
        )
        .await;
    }
    // 级联回收回帖 +1：主题删除会把全部回帖 CASCADE 掉，逐笔回收回帖奖励保持对称。
    // 审计修复（P1 双重扣分）：楼主首帖也存于 posts（topic_create 落 posts 行），
    // 旧版把它计为「回帖」再 -1，楼主实扣 -3（发帖 -2 + 首帖按回帖 -1）。
    // 排除楼主首帖，楼主只按发帖口径 -2。
    let replies =
        repo_td::reply_reward_debt(&state.repo, tid, op.unwrap_or(0)).await;
    for (uid, n) in replies {
        let _ = earn_spark(
            &state.repo.db,
            uid,
            -(n as i64),
            "forum-topic-del-reply",
            &format!("forum-topic-del-reply:{tid}:{uid}"),
        )
        .await;
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
