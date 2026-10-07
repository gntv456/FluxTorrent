//! M15 论坛互动（点赞/收藏）。
//! 从 community_http.rs 按域拆出。

use actix_web::{delete, post, web, HttpRequest, HttpResponse};

use super::*;
use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ---- 论坛互动：点赞 / 收藏（0116） ----

/// 点赞/取消点赞共用的落库逻辑。
/// 幂等：post_likes 主键 (user_id, post_id) + ON CONFLICT DO NOTHING。
/// 火花：仅「非本人作者 + 本次真发生变化 + 点赞方向」时给作者 +1（幂等键 forum-like:{pid}:{fan}），
/// 取消点赞不回收 —— 因为幂等键去重已堵住 like/unlike 循环刷分，回收反而会算错。
async fn set_post_like(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &crate::http::AuthUser,
    pid: i64,
    on: bool,
) -> DomainResult<HttpResponse> {
    let row: Option<(i64, i64, i64, String)> = sqlx::query_as(
        "SELECT p.topic_id, t.forum_id, COALESCE(p.user_id, 0), t.title \
         FROM posts p JOIN topics t ON t.id = p.topic_id WHERE p.id = $1",
    )
    .bind(pid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((tid, fid, author, title)) = row else {
        return Err(DomainError::NotFound(pid));
    };
    let perm =
        forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_read {
        return Err(DomainError::Forbidden);
    }
    let changed = if on {
        sqlx::query(
            "INSERT INTO post_likes (user_id, post_id, topic_id) VALUES ($1, $2, $3) \
             ON CONFLICT (user_id, post_id) DO NOTHING",
        )
        .bind(auth.id)
        .bind(pid)
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected()
            > 0
    } else {
        sqlx::query(
            "DELETE FROM post_likes WHERE user_id = $1 AND post_id = $2",
        )
        .bind(auth.id)
        .bind(pid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected()
            > 0
    };
    if on && changed && author != auth.id && author != 0 {
        // 二轮遗留（2026-10-07）：点赞奖励加账号年龄门槛——小号矩阵对新帖
        // 逐帖互赞每对 (post, liker) +1 火力，幂等键只防重复不防「新号海量」。
        // 注册满 7 天才计奖（点赞行照常记录，仅火花不发）；同 IP 检测交给
        // 事后 ipcheck（实时 JOIN login_events 成本过高，不在点赞路径做）。
        let liker_age_ok: bool = sqlx::query_scalar(
            "SELECT (now() - created_at) >= interval '7 days'              FROM users WHERE id = $1",
        )
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        if !liker_age_ok {
            let likes: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM post_likes WHERE post_id = $1",
            )
            .bind(pid)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            return Ok(ok(serde_json::json!({
                "liked": true, "likes": likes, "reward": false,
            })));
        }
        let idem = format!("forum-like:{}:{}", pid, auth.id);
        // 通知与火花共用同一幂等键：`changed=true` 只保证「本次真的写入了点赞行」，
        // 但「取消后再点赞」同样会 changed=true。若不按幂等键判重，
        // like/unlike 循环就能反复给作者刷通知（火花侧已有幂等键，通知侧漏了）。
        let already: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE \
             idempotency_key = $1)",
        )
        .bind(&idem)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        let _ =
            earn_spark(&state.repo.db, author, 1, "forum-like", &idem).await;
        if !already {
            notify_user(
                &state.repo.db,
                author,
                "帖子被点赞",
                &format!(
                    "用户 #{} 赞了你在主题「{}」里的回复：[查看](/forums/topic/{})",
                    auth.id, title, tid
                ),
            )
            .await;
        }
    }
    let likes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM post_likes WHERE post_id = $1",
    )
    .bind(pid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "post_id": pid,
        "liked": on,
        "likes": likes,
        "changed": changed,
    })))
}

#[post("/forums/posts/{id}/like")]
async fn post_like(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let pid = path.into_inner();
    set_post_like(&state, &auth, pid, true).await
}

#[delete("/forums/posts/{id}/like")]
async fn post_unlike(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let pid = path.into_inner();
    set_post_like(&state, &auth, pid, false).await
}

/// 收藏/取消收藏主题共用的落库逻辑（幂等，主键 (user_id, topic_id)）
async fn set_topic_favorite(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &crate::http::AuthUser,
    tid: i64,
    on: bool,
) -> DomainResult<HttpResponse> {
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
    if !perm.can_read {
        return Err(DomainError::Forbidden);
    }
    if on {
        sqlx::query(
            "INSERT INTO topic_favorites (user_id, topic_id) VALUES ($1, $2) \
             ON CONFLICT (user_id, topic_id) DO NOTHING",
        )
        .bind(auth.id)
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    } else {
        sqlx::query(
            "DELETE FROM topic_favorites WHERE user_id = $1 AND topic_id = $2",
        )
        .bind(auth.id)
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    let favorites: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM topic_favorites WHERE topic_id = $1",
    )
    .bind(tid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "topic_id": tid,
        "faved": on,
        "favorites": favorites,
    })))
}

#[post("/forums/topics/{id}/favorite")]
async fn topic_favorite(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    set_topic_favorite(&state, &auth, tid, true).await
}

#[delete("/forums/topics/{id}/favorite")]
async fn topic_unfavorite(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    set_topic_favorite(&state, &auth, tid, false).await
}
