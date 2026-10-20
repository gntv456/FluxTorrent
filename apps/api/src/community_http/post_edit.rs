//! M15 帖子编辑/删除（post_edit/post_delete）。
//! 从 community_http.rs 按域拆出。

use actix_web::{delete, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::*;
use crate::dto::ok;
use crate::economy_http::clawback_spark;
use crate::repo::community::forum_reward;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize, utoipa::ToSchema)]
struct PostEditReq {
    body: String,
}

#[utoipa::path(
    put,
    path = "/api/v1/forums/posts/{id}",
    tag = "社区/论坛",
    description = "改自己的回帖（作者或版主）。",
    params(
        ("id" = i64, description = "回帖 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 403, description = "非作者且无编辑权（code 2003）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[put("/forums/posts/{id}")]
pub(crate) async fn post_edit(
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
    // P-54：视频上限同样要在编辑口过闸——旧版只有发帖/回帖查，
    // 「先发一条、再改成十条」把 post_video_max 直接抹平。
    check_video_count(&state.repo.db, &body.body).await?;
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
    let n = sqlx::query(
        "UPDATE posts SET body = $1, body_text = $4, \
         edited_at = now(), edited_by = $2 WHERE id = $3",
    )
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
            "INSERT INTO messages (sender_id, receiver_id, \
             subject, body) VALUES ($1, $2, $3, $4)",
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
#[utoipa::path(
    delete,
    path = "/api/v1/forums/posts/{id}",
    tag = "社区/社区",
    description = "删回帖（作者本人；删主题走 /forums/topics/{id}）。",
    params(
        ("id" = i64, description = "回帖 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 403, description = "非作者且无删帖权（code 2003）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[delete("/forums/posts/{id}")]
pub(crate) async fn post_delete(
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
    // 删单帖收回这一帖当初真拿到的回帖奖励（与删主题收回发帖奖励对称，
    // 堵住「发帖赚火花再被删」的获利通道）
    {
        // P-415：收回走地板原语。旧写法 `let _ = earn_spark(…, -1, …)` 把 -1 送进
        // 发放原语（`checked_balance` 只防溢出不防负、`spark_balance` 无 CHECK），
        // 零余额会员被删一帖就变成「欠站里 1 火花」，失败还被 `let _ =` 吞干净。
        // P-456：收回额回查流水，不再写死 1。写死有两处错——面板 makepost 调成
        // 3 之后老帖按 1 发、按 3 收（多扣）；楼主首帖拿的是发帖奖励、从来没有
        // 回帖流水，单独删首帖会被凭空扣 1。查不到流水＝当时没发过，本就没有
        // 可收回的，此时必须跳过：clawback 原语对 <=0 直接判 Err（负数请走扣款
        // 原语），硬塞 0 只会刷「收回失败」的假告警。
        let owed = match forum_reward::granted_by_idem(
            &state.repo,
            &forum_reward::reply_idem(author_id, pid),
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    err = %e,
                    post = pid,
                    "删帖前回查回帖发放流水失败：本笔不收回"
                );
                0
            }
        };
        if owed > 0 {
            if let Err(e) = clawback_spark(
                &state.repo.db,
                author_id,
                owed,
                "forum-post-del",
                &format!("forum-post-del:{pid}"),
            )
            .await
            {
                tracing::warn!(
                    err = %e,
                    post = pid,
                    user = author_id,
                    "删帖收回火花失败"
                );
            }
        }
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
