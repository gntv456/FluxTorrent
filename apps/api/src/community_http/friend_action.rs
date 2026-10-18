//! 好友操作（POST /friends/action）从 friend.rs 按域拆出。
//! friend.rs 加 utoipa 注解后贴近 300 行门禁，按域拆出。
//!
//! 2026-10-16：SQL 已下沉到 repo::community::friends（H 层不 import
//! sqlx，FRAME §2）。本文件只留「能不能这么做」的口径判断与 JSON 组装。

use super::friend::FriendActionReq;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::repo::community::friends as repo_friends;
use crate::state::AppState;
use actix_web::{post, web, HttpRequest, HttpResponse};

/// 好友操作：拉黑/取消拉黑入口（此前黑名单只有消费端没有生产端，PM 拦截成死代码）。
/// 拉黑语义：upsert 我为 user_id 的行 list='black'；并撤回我对对方的 pending 申请。
#[utoipa::path(
    post,
    path = "/api/v1/friends/action",
    tag = "社区/好友",
    description = "好友操作（同意/拒绝/删除）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[post("/friends/action")]
pub(crate) async fn friend_action(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FriendActionReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let Some(fid) =
        repo_friends::find_live_user_id(&state.repo, &body.username).await?
    else {
        return Err(DomainError::NotFound(0));
    };
    if fid == auth.id {
        return Err(DomainError::Validation("不能对自己操作".into()));
    }
    match body.action.as_str() {
        "black" => {
            repo_friends::set_black(&state.repo, auth.id, fid).await?;
            repo_friends::drop_my_pending(&state.repo, auth.id, fid).await?;
            Ok(ok(
                serde_json::json!({ "user": body.username, "state": "black" }),
            ))
        }
        "unblack" => {
            let n =
                repo_friends::unset_black(&state.repo, auth.id, fid).await?;
            Ok(ok(
                serde_json::json!({ "user": body.username, "removed": n }),
            ))
        }
        // 接受收到的申请（对方→我的 pending）。界面「接受」按钮走这条，
        // 与 friend_create 的互转正路径同口径（含给对方的系统信）。
        "accept" => {
            let they_pending =
                repo_friends::has_their_pending(&state.repo, auth.id, fid)
                    .await?;
            if !they_pending {
                return Err(DomainError::Validation(
                    "没有待处理的好友申请".into(),
                ));
            }
            repo_friends::mark_both_friend(&state.repo, auth.id, fid).await?;
            repo_friends::ensure_my_side_friend(&state.repo, auth.id, fid)
                .await?;
            repo_friends::notify_friend_accepted(&state.repo, fid, auth.id)
                .await;
            Ok(ok(
                serde_json::json!({ "user": body.username, "state": "friend" }),
            ))
        }
        // 拒绝 = 只撤掉对方发来的那条申请，不通知（NP 同口径）
        "reject" => {
            let n =
                repo_friends::reject_their_pending(&state.repo, auth.id, fid)
                    .await?;
            Ok(ok(serde_json::json!({
                "user": body.username, "rejected": n as i64,
            })))
        }
        _ => Err(DomainError::Validation(
            "action 需为 black / unblack / accept / reject".into(),
        )),
    }
}
