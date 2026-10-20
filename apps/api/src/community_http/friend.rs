//! M16 好友（add/list/action）。
//! 从 community_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::repo::community::actor::actor_name;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ 好友（M16） ============

#[derive(Deserialize, utoipa::ToSchema)]
struct FriendReq {
    username: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/friends",
    tag = "社区/好友",
    description = "加好友申请。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[post("/friends")]
pub(crate) async fn friend_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FriendReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let fid: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE username = $1 AND status < 2",
    )
    .bind(&body.username)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .flatten();
    let Some(fid) = fid else {
        return Err(DomainError::NotFound(0));
    };
    if fid == auth.id {
        return Err(DomainError::Validation("不能添加自己".into()));
    }
    // 审计修复（P1 隐私）：旧版单方 INSERT 即成好友，可绕过 accept_pm='friends' 屏障。
    // 新流程：对方拉黑则拒绝；对方已申请我 → 双向转正为好友（接受）；否则写 pending 申请并通知。
    let blacklisted: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM friendships WHERE user_id = $2 \
         AND friend_id = $1 AND list = 'black')",
    )
    .bind(auth.id)
    .bind(fid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if blacklisted {
        return Err(DomainError::Validation("对方拒绝了你的好友申请".into()));
    }
    let they_pending: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM friendships WHERE user_id = $2 \
         AND friend_id = $1 AND list = 'pending')",
    )
    .bind(auth.id)
    .bind(fid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if they_pending {
        // 对方先申请过我：双向转正
        sqlx::query("UPDATE friendships SET list = 'friend' WHERE \
         (user_id = $1 AND friend_id = $2) OR (user_id = $2 AND friend_id = $1)")
            .bind(auth.id)
            .bind(fid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, \
             subject, body) VALUES (NULL, $1, $2, $3)",
        )
        .bind(fid)
        .bind("好友申请已通过")
        .bind(format!("{} 接受了你的好友申请，你们现在是好友了。",
                      actor_name(&state.repo.db, auth.id).await))
        .execute(&state.repo.db)
        .await;
        return Ok(ok(
            serde_json::json!({ "friend": body.username, "state": "friend" }),
        ));
    }
    sqlx::query(
        "INSERT INTO friendships (user_id, friend_id, list) VALUES \
     ($1, $2, 'pending') ON CONFLICT DO NOTHING",
    )
    .bind(auth.id)
    .bind(fid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let _ = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) \
         VALUES (NULL, $1, $2, $3)",
    )
    .bind(fid)
    .bind("收到好友申请")
    .bind(format!("{} 向你发送了好友申请。添加对方为好友即可接受。",
                  actor_name(&state.repo.db, auth.id).await))
    .execute(&state.repo.db)
    .await;
    Ok(ok(
        serde_json::json!({ "friend": body.username, "state": "pending" }),
    ))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct FriendRow {
    username: String,
    /// friend | black | incoming（他人发给我的 pending 申请）
    list: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/friends",
    tag = "社区/好友",
    description = "好友列表（互关状态）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/friends")]
pub(crate) async fn friend_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 双向口径：我发出的行（user_id=我，含 pending/black）+ 他人发给我的 pending 申请（list='incoming'）。
    // 旧版只查 user_id=$1，被申请方永远看不到收到的申请 → pending 流程发得出、收不到。
    // 行必须是**对象**：`friends-actions.tsx` 的 FriendRow 读 r.username/r.list，
    // 早前这里返回 (String,String) 元组，序列化成 [["bob","incoming"]]，
    // r.list 恒为 undefined → 「收到的好友申请」整段不渲染，接受/拒绝无从点起。
    let rows: Vec<FriendRow> = sqlx::query_as(
        "SELECT u.username, f.list FROM friendships f JOIN users u ON u.id = f.friend_id WHERE f.user_id = $1 \
         UNION ALL \
         SELECT u.username, 'incoming' FROM friendships f JOIN users u ON u.id = f.user_id \
         WHERE f.friend_id = $1 AND f.list = 'pending' \
         ORDER BY 1 LIMIT 400",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub(crate) struct FriendActionReq {
    pub(crate) username: String,
    /// black = 拉黑；unblack = 取消拉黑
    pub(crate) action: String,
}
