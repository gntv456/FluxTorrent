//! me 角标聚合（M16 后续）：头像下拉工具行的四个可操作计数。
//! 从 auth_http 按域拆出。

use actix_web::{get, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::state::AppState;

/// 工具行角标聚合（user-tools 口径，一次请求取回四个计数）：
/// - unread：收件箱未读（与 /me 同口径 `location=1 AND folder IS NULL`）
/// - tickets：我的工单「已答复待确认」（ticket_status=2）——用户可确认的待办
/// - friend_requests：收到的好友申请（incoming pending）
/// - staff：员工视角三队列（无 staff.message 权时为 null 不硬 403）——
///   {reports: 待处理举报, cheaters: 作弊嫌疑会话, staffbox: 未答复工单 }
///
/// 失败语义：单计数查询失败记 0 不炸整包（角标是锦上添花，不该把整行
/// 工具拖红）；staff 分支无权限时静默置 null，前端不渲染员工入口数字。
#[utoipa::path(
    get,
    path = "/api/v1/me/badges",
    tag = "我的",
    description = "工具行角标聚合：未读私信/工单待确认/好友申请/员工三队列。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/me/badges")]
pub async fn me_badges(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;
    let unread = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM messages WHERE receiver_id = $1 AND \
         unread = true AND location = 1 AND folder IS NULL",
    )
    .bind(auth.id)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let tickets = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM staffmessages WHERE user_id = $1 \
         AND ticket_status = 2",
    )
    .bind(auth.id)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let friend_requests = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM friendships WHERE friend_id = $1 \
         AND list = 'pending'",
    )
    .bind(auth.id)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    // 员工三队列：无 staff.message 权限整个分支持 null（普通用户照常拿到
    // 前三个计数；不是把 403 演成 0——0 会暗示「队列处理完了」）
    let staff = if crate::authz::can(
        &state,
        &auth,
        crate::authz::perm::STAFF_MESSAGE,
    )
    .await
    {
        let reports = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM reports WHERE status = 0",
        )
        .fetch_one(db)
        .await
        .unwrap_or(0);
        let cheaters = sqlx::query_scalar::<_, i64>(
            // 与 /admin/cheaters 同口径（近 1 小时增量/速度双阈值）
            "SELECT count(*) FROM snatches s \
             JOIN LATERAL (SELECT COALESCE(sum(tl.delta_up), 0)::bigint AS d \
              FROM traffic_ledger tl WHERE tl.user_id = s.user_id \
              AND tl.torrent_id = s.torrent_id \
              AND tl.window_start > now() - interval '1 hour') g ON TRUE \
             WHERE (s.seeding OR s.leeching) \
             AND (g.d > 536870912000 OR g.d / 3600 > 104857600)",
        )
        .fetch_one(db)
        .await
        .unwrap_or(0);
        let staffbox = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM staffmessages WHERE answered = 0",
        )
        .fetch_one(db)
        .await
        .unwrap_or(0);
        serde_json::json!({
            "reports": reports,
            "cheaters": cheaters,
            "staffbox": staffbox,
        })
    } else {
        serde_json::Value::Null
    };
    Ok(ok(serde_json::json!({
        "unread": unread,
        "tickets": tickets,
        "friend_requests": friend_requests,
        "staff": staff,
    })))
}
