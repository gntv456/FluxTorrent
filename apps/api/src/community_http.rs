//! 社区模块 HTTP 接口（M14 勋章 + M15 论坛 + M16 短讯/好友）。

use actix_web::{get, post, put, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use uuid::Uuid;

use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

pub fn mount_community(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        // M14 勋章
        .service(medal_list)
        .service(medal_buy)
        .service(medal_gift)
        .service(medal_wear)
        .service(my_medals)
        // M15 论坛
        .service(forum_list)
        .service(topic_create)
        .service(topic_list)
        .service(topic_detail)
        .service(post_reply)
        // M16 短讯与好友
        .service(message_send)
        .service(message_inbox)
        .service(message_sent)
        .service(friend_add)
        .service(friend_list)
}

// ============ M14 勋章 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct MedalRow {
    id: i64,
    name: String,
    price: Option<i64>,
    rarity: Option<String>,
    limited: bool,
    owned: bool,
    wearing: bool,
}

#[get("/medals")]
async fn medal_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await.ok();
    let uid = auth.map(|a| a.id);
    let rows = sqlx::query_as::<_, MedalRow>(
        "SELECT m.id, m.name, m.price, m.rarity, m.limited, \
            ($1::bigint IS NOT NULL AND EXISTS(SELECT 1 FROM user_medals um WHERE um.medal_id = m.id AND um.user_id = $1)) AS owned, \
            ($1::bigint IS NOT NULL AND EXISTS(SELECT 1 FROM user_medals um WHERE um.medal_id = m.id AND um.user_id = $1 AND um.wearing)) AS wearing \
         FROM medals m ORDER BY m.id",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct MedalBuyReq {
    medal_id: i64,
}

#[post("/medals/buy")]
async fn medal_buy(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MedalBuyReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let price: Option<i64> = sqlx::query_scalar("SELECT price FROM medals WHERE id = $1")
        .bind(body.medal_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .flatten();
    let Some(price) = price else {
        return Err(DomainError::NotFound(body.medal_id)); // 非卖品勋章（如开站勋章）
    };
    // 已拥有直接拒绝（防重复扣款）
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_medals WHERE user_id = $1 AND medal_id = $2)",
    )
    .bind(auth.id)
    .bind(body.medal_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if owned {
        return Err(DomainError::Validation("已拥有该勋章".into()));
    }
    let idem = format!("medal-buy:{}:{}:{}", auth.id, body.medal_id, Uuid::new_v4());
    crate::economy_http::spend_spark(
        &state.repo.db,
        auth.id,
        price,
        "shop",
        &idem,
        "medal",
        body.medal_id,
    )
    .await?;
    sqlx::query("INSERT INTO user_medals (user_id, medal_id, source) VALUES ($1, $2, 'buy') ON CONFLICT DO NOTHING")
        .bind(auth.id)
        .bind(body.medal_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "medal_id": body.medal_id, "price": price }),
    ))
}

#[derive(Deserialize)]
struct MedalGiftReq {
    medal_id: i64,
    to_user: String,
}

#[post("/medals/gift")]
async fn medal_gift(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MedalGiftReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let to_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM users WHERE username = $1 AND status < 2")
            .bind(&body.to_user)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
    let Some(to_id) = to_id else {
        return Err(DomainError::NotFound(0));
    };
    if to_id == auth.id {
        return Err(DomainError::Validation("不能赠送给自己".into()));
    }
    // 购买并直接入对方账户（赠送弹窗流程：一步完成）
    let price: Option<i64> = sqlx::query_scalar("SELECT price FROM medals WHERE id = $1")
        .bind(body.medal_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .flatten();
    let Some(price) = price else {
        return Err(DomainError::NotFound(body.medal_id));
    };
    let idem = format!(
        "medal-gift:{}:{}:{}",
        auth.id,
        body.medal_id,
        Uuid::new_v4()
    );
    crate::economy_http::spend_spark(
        &state.repo.db,
        auth.id,
        price,
        "shop",
        &idem,
        "medal_gift",
        body.medal_id,
    )
    .await?;
    sqlx::query("INSERT INTO user_medals (user_id, medal_id, source) VALUES ($1, $2, 'gift') ON CONFLICT DO NOTHING")
        .bind(to_id)
        .bind(body.medal_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "to": body.to_user, "medal_id": body.medal_id }),
    ))
}

#[derive(Deserialize)]
struct WearReq {
    medal_id: Option<i64>,
}

#[put("/medals/wear")]
async fn medal_wear(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WearReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 单佩戴位：先全部摘下
    sqlx::query("UPDATE user_medals SET wearing = false WHERE user_id = $1")
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(mid) = body.medal_id {
        let updated = sqlx::query(
            "UPDATE user_medals SET wearing = true WHERE user_id = $1 AND medal_id = $2",
        )
        .bind(auth.id)
        .bind(mid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if updated.rows_affected() == 0 {
            return Err(DomainError::Validation("未拥有该勋章".into()));
        }
    }
    Ok(ok(serde_json::json!({ "wearing": body.medal_id })))
}

#[get("/me/medals")]
async fn my_medals(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<(i64, String, bool)> = sqlx::query_as(
        "SELECT m.id, m.name, um.wearing FROM user_medals um JOIN medals m ON m.id = um.medal_id WHERE um.user_id = $1 ORDER BY m.id",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ M15 论坛 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct ForumRow {
    id: i64,
    name: String,
    descr: Option<String>,
    topics: i64,
    posts: i64,
}

#[get("/forums")]
async fn forum_list(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let rows = sqlx::query_as::<_, ForumRow>(
        "SELECT f.id, f.name, f.descr, \
            (SELECT count(*) FROM topics t WHERE t.forum_id = f.id) AS topics, \
            (SELECT count(*) FROM posts p JOIN topics t ON t.id = p.topic_id WHERE t.forum_id = f.id) AS posts \
         FROM forums f WHERE f.min_class <= 99 ORDER BY f.id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct TopicCreateReq {
    forum_id: i64,
    title: String,
    body: String,
}

#[post("/forums/topics")]
async fn topic_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TopicCreateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.title.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("标题与正文不能为空".into()));
    }
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let topic_id: i64 = sqlx::query_scalar(
        "INSERT INTO topics (forum_id, user_id, title) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(body.forum_id)
    .bind(auth.id)
    .bind(&body.title)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("INSERT INTO posts (id, topic_id, user_id, body) VALUES (nextval('posts_id_seq'), $1, $2, $3)")
        .bind(topic_id)
        .bind(auth.id)
        .bind(&body.body)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("UPDATE topics SET last_post_at = now() WHERE id = $1")
        .bind(topic_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 发主题 +2 火花（M11 论坛行为奖励）
    let idem = format!("forum-topic:{}:{}", auth.id, topic_id);
    earn_spark(&state.repo.db, auth.id, 2, "forum", &idem).await?;
    Ok(ok(serde_json::json!({ "topic_id": topic_id })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct TopicRow {
    id: i64,
    forum_id: i64,
    title: String,
    username: Option<String>,
    replies: i64,
    views: i32,
    last_post_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[get("/forums/{id}/topics")]
async fn topic_list(
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let rows = sqlx::query_as::<_, TopicRow>(
        "SELECT t.id, t.forum_id, t.title, u.username, \
            (SELECT count(*)-1 FROM posts p WHERE p.topic_id = t.id) AS replies, t.views, t.last_post_at \
         FROM topics t LEFT JOIN users u ON u.id = t.user_id \
         WHERE t.forum_id = $1 AND NOT t.locked ORDER BY t.sticky DESC, t.id DESC LIMIT 50",
    )
    .bind(path.into_inner())
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct PostRow {
    id: i64,
    username: Option<String>,
    body: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/forums/topics/{id}")]
async fn topic_detail(
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let tid = path.into_inner();
    sqlx::query("UPDATE topics SET views = views + 1 WHERE id = $1")
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let posts = sqlx::query_as::<_, PostRow>(
        "SELECT p.id, u.username, p.body, p.created_at FROM posts p \
         LEFT JOIN users u ON u.id = p.user_id WHERE p.topic_id = $1 ORDER BY p.id LIMIT 200",
    )
    .bind(tid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(posts))
}

#[derive(Deserialize)]
struct ReplyReq {
    body: String,
}

#[post("/forums/topics/{id}/reply")]
async fn post_reply(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ReplyReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    if body.body.trim().is_empty() {
        return Err(DomainError::Validation("回复不能为空".into()));
    }
    let locked: Option<bool> = sqlx::query_scalar("SELECT locked FROM topics WHERE id = $1")
        .bind(tid)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .flatten();
    match locked {
        None => return Err(DomainError::NotFound(tid)),
        Some(true) => return Err(DomainError::Validation("主题已锁定".into())),
        Some(false) => {}
    }
    let post_id: i64 = sqlx::query_scalar(
        "INSERT INTO posts (id, topic_id, user_id, body) VALUES (nextval('posts_id_seq'), $1, $2, $3) RETURNING id",
    )
    .bind(tid)
    .bind(auth.id)
    .bind(&body.body)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("UPDATE topics SET last_post_at = now() WHERE id = $1")
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 回帖 +1 火花
    let idem = format!("forum-reply:{}:{}", auth.id, post_id);
    earn_spark(&state.repo.db, auth.id, 1, "forum", &idem).await?;
    Ok(ok(serde_json::json!({ "post_id": post_id })))
}

// ============ M16 短讯与好友 ============

#[derive(Deserialize)]
struct SendMsgReq {
    to: String,
    subject: String,
    body: String,
}

#[post("/messages")]
async fn message_send(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SendMsgReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let to_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM users WHERE username = $1 AND status < 2")
            .bind(&body.to)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
    let Some(to_id) = to_id else {
        return Err(DomainError::NotFound(0));
    };
    if body.subject.trim().is_empty() {
        return Err(DomainError::Validation("主题不能为空".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(auth.id)
    .bind(to_id)
    .bind(&body.subject)
    .bind(&body.body)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct MessageRow {
    id: i64,
    counterpart: Option<String>,
    subject: String,
    body: String,
    read_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/messages/inbox")]
async fn message_inbox(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT m.id, u.username AS counterpart, m.subject, m.body, m.read_at, m.created_at \
         FROM messages m LEFT JOIN users u ON u.id = m.sender_id \
         WHERE m.receiver_id = $1 ORDER BY m.id DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[get("/messages/sent")]
async fn message_sent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT m.id, u.username AS counterpart, m.subject, m.body, m.read_at, m.created_at \
         FROM messages m LEFT JOIN users u ON u.id = m.receiver_id \
         WHERE m.sender_id = $1 ORDER BY m.id DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ 好友（M16） ============

#[derive(Deserialize)]
struct FriendReq {
    username: String,
}

#[post("/friends")]
async fn friend_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FriendReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let fid: Option<i64> =
        sqlx::query_scalar("SELECT id FROM users WHERE username = $1 AND status < 2")
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
    sqlx::query("INSERT INTO friendships (user_id, friend_id, list) VALUES ($1, $2, 'friend') ON CONFLICT DO NOTHING")
        .bind(auth.id)
        .bind(fid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "friend": body.username })))
}

#[get("/friends")]
async fn friend_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT u.username, f.list FROM friendships f JOIN users u ON u.id = f.friend_id WHERE f.user_id = $1 ORDER BY u.username LIMIT 200",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 主配置：主 scope + 经济路由 + 社区路由（单一 /api/v1 scope）
pub fn configure(cfg: &mut web::ServiceConfig) {
    let scope = crate::economy_http::mount_economy(crate::http::v1_scope());
    let scope = mount_community(scope);
    let scope = crate::games_http::mount_games(crate::ops_http::mount_ops(
        crate::content_http::mount_content(scope),
    ));
    let scope = crate::admin_http::mount_admin(scope);
    cfg.service(crate::openapi_http::mount_openapi(scope));
}
