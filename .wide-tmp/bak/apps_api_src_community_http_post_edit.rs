//! M15 帖子编辑/删除（post_edit/post_delete）。
//! 从 community_http.rs 按域拆出。

use actix_web::{delete, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::*;
use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
struct PostEditReq {
    body: String,
}

#[put("/forums/posts/{id}")]
async fn post_edit(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PostEditReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.body.trim().is_empty() {
        return Err(DomainError::Validation("正文不能为空".into()));
    }
    // 敏感词（Phase3）：编辑同样过闸（防止先发合规后改敏感词绕过）
    check_banned_words(&state.repo.db, &body.body).await?;
    let pid = path.into_inner();
    let Some((_tid, fid, author_id)) =
        post_context(&state.repo.db, pid).await?
    else {
        return Err(DomainError::NotFound(pid));
    };
    let perm =
        forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_mod && author_id != auth.id {
        return Err(DomainError::Forbidden);
    }
    let n =
        sqlx::query("UPDATE posts SET body = $1, body_text = $4, edited_at = now(), edited_by = $2 WHERE id = $3")
            .bind(body.body.trim())
            .bind(auth.id)
            .bind(pid)
            .bind(strip_markdown(body.body.trim()))
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(pid));
    }
    // 管理员/版主编辑他人帖：自动 PM 通知作者（forums.php:426 口径）
    if author_id != auth.id {
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES ($1, $2, $3, $4)",
        )
        .bind(auth.id)
        .bind(author_id)
        .bind("您的帖子被编辑")
        .bind(format!("您的帖子已被管理组成员编辑，请查看最新内容。"))
        .execute(&state.repo.db)
        .await;
    }
    Ok(ok(serde_json::json!({ "edited": pid })))
}

/// 删帖：仅版主/postmanage —— 普通用户（含作者本人）删不掉自己的帖，想删找版主
#[delete("/forums/posts/{id}")]
async fn post_delete(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let pid = path.into_inner();
    let Some((tid, fid, author_id)) = post_context(&state.repo.db, pid).await?
    else {
        return Err(DomainError::NotFound(pid));
    };
    let perm =
        forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_mod {
        return Err(DomainError::Forbidden);
    }
    // 删单帖收回该帖的回帖 +1（与删主题收回发帖 +2 对称，堵住「发帖赚火花再被删」的获利通道）
    {
        let _ = earn_spark(
            &state.repo.db,
            author_id,
            -1,
            "forum-post-del",
            &format!("forum-post-del:{pid}"),
        )
        .await;
    }
    // 审计修复（P2）：被删帖若是该主题最新回复，last_post_at 残留已删时间——
    // 版块「最后回复」排序/展示失真。先取被删帖时间，删后回填剩余最新回复
    // （无回复则回落主题创建时间）。
    let deleted_at: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT created_at FROM posts WHERE id = $1 AND topic_id = $2",
    )
    .bind(pid)
    .bind(tid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .flatten();
    sqlx::query("DELETE FROM posts WHERE id = $1 AND topic_id = $2")
        .bind(pid)
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 点赞行无 FK（posts 为分区表，无法对 post_id 建外键）：单帖删除时手动清理，避免孤儿点赞
    let _ = sqlx::query("DELETE FROM post_likes WHERE post_id = $1")
        .bind(pid)
        .execute(&state.repo.db)
        .await;
    if let Some(d) = deleted_at {
        let _ = sqlx::query(
            "UPDATE topics t SET last_post_at = COALESCE( \
                (SELECT max(created_at) FROM posts p WHERE p.topic_id = t.id), t.created_at) \
             WHERE t.id = $1 AND t.last_post_at = $2",
        )
        .bind(tid)
        .bind(d)
        .execute(&state.repo.db)
        .await;
    }
    state
        .repo
        .audit(Some(auth.id), "forum.post_delete", Some(pid))
        .await;
    Ok(ok(serde_json::json!({ "deleted": pid })))
}

/// 删主题：仅版主/postmanage；收回发帖 +2 火花（KPS("-", starttopic_bonus) 口径）
#[delete("/forums/topics/{id}")]
async fn topic_delete(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    let fid: Option<i64> =
        sqlx::query_scalar("SELECT forum_id FROM topics WHERE id = $1")
            .bind(tid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(fid) = fid else {
        return Err(DomainError::NotFound(tid));
    };
    let perm =
        forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_mod {
        return Err(DomainError::Forbidden);
    }
    let op: Option<i64> =
        sqlx::query_scalar("SELECT user_id FROM topics WHERE id = $1")
            .bind(tid)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    // 悬赏未决退回（0124）：open 状态的悬赏在删主题时退还楼主（已 awarded 的不动——钱已归答主）。
    // 幂等键与发放错开：`-refund` 后缀，退回与发放都各只发生一次。
    if let Some(op_id) = op {
        let bounty_open: Option<i64> = sqlx::query_scalar(
            "SELECT bounty_spark FROM topics WHERE id = $1 AND topic_type = 'bounty' \
             AND bounty_status = 'open' AND bounty_spark > 0",
        )
        .bind(tid)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if let Some(refund) = bounty_open {
            let _ = earn_spark(
                &state.repo.db,
                op_id,
                refund,
                "forum_bounty_refund",
                &format!("forum-bounty-refund:{tid}"),
            )
            .await;
            let _ = sqlx::query(
                "UPDATE topics SET bounty_status = 'refunded' WHERE id = $1 AND bounty_status = 'open'",
            )
            .bind(tid)
            .execute(&state.repo.db)
            .await;
        }
        // 抽奖未决退回（0126）：open 状态的抽奖在删主题时奖金池退还楼主（drawn 的不动——钱已归中奖人）。
        // 票价不退（参与者享受了参与过程；与删悬赏帖不追回已发赏金同一不对称口径）。
        let lot_open: Option<i64> = sqlx::query_scalar(
            "SELECT winners::bigint * prize_per_winner FROM topic_lotteries \
             WHERE topic_id = $1 AND status = 'open' AND prize_per_winner > 0",
        )
        .bind(tid)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if let Some(refund) = lot_open {
            let _ = earn_spark(
                &state.repo.db,
                op_id,
                refund,
                "forum_lottery_refund",
                &format!("forum-lottery-refund:{tid}"),
            )
            .await;
            let _ = sqlx::query(
                "UPDATE topic_lotteries SET status = 'cancelled' WHERE topic_id = $1 AND status = 'open'",
            )
            .bind(tid)
            .execute(&state.repo.db)
            .await;
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
    let replies: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT DISTINCT user_id, count(*) OVER (PARTITION BY user_id) FROM posts \
         WHERE topic_id = $1 AND NOT (user_id = $2 AND id = (SELECT min(id) FROM posts WHERE topic_id = $1))",
    )
    .bind(tid)
    .bind(op.unwrap_or(0))
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
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
    sqlx::query("DELETE FROM topics WHERE id = $1")
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 关注本主题的行：target_id 无外键（指向 3 张表），必须应用层清理，否则留下悬空关注
    let _ = sqlx::query(
        "DELETE FROM follows WHERE target_type = 'topic' AND target_id = $1",
    )
    .bind(tid)
    .execute(&state.repo.db)
    .await;
    state
        .repo
        .audit(Some(auth.id), "forum.topic_delete", Some(tid))
        .await;
    Ok(ok(serde_json::json!({ "deleted": tid })))
}
