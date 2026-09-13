//! 社区模块 HTTP 接口（M14 勋章 + M15 论坛 + M16 短讯/好友）。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse, Responder};
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
        .service(post_edit)
        .service(post_delete)
        .service(topic_delete)
        .service(topic_manage)
        // M16 短讯与好友
        .service(message_send)
        .service(message_markread)
        .service(message_delete)
        .service(message_move)
        .service(message_boxes)
        .service(message_box_upsert)
        .service(contact_staff)
        .service(staff_messages)
        .service(staff_answer)
        .service(staff_mark)
        .service(staff_delete)
        .service(shoutbox_list)
        .service(shoutbox_send)
        .service(message_inbox)
        .service(message_sent)
        .service(message_staff)
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
    description: Option<String>,
    duration_days: Option<i32>,
    get_type: i16,
    sale_begin_at: Option<chrono::DateTime<chrono::Utc>>,
    sale_end_at: Option<chrono::DateTime<chrono::Utc>>,
    inventory: Option<i32>,
    bonus_addition_factor: f64,
    category_id: i32,
    category_name: Option<String>,
}

#[get("/medals")]
async fn medal_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await.ok();
    let uid = auth.map(|a| a.id);
    let rows = sqlx::query_as::<_, MedalRow>(
        "SELECT m.id, m.name, m.price, m.rarity, m.limited, m.description, m.duration_days, m.get_type,             m.sale_begin_at, m.sale_end_at, m.inventory, m.bonus_addition_factor::float8, m.category_id,             c.name AS category_name,             ($1::bigint IS NOT NULL AND EXISTS(SELECT 1 FROM user_medals um WHERE um.medal_id = m.id AND um.user_id = $1 AND (um.expires_at IS NULL OR um.expires_at > now()))) AS owned,             ($1::bigint IS NOT NULL AND EXISTS(SELECT 1 FROM user_medals um WHERE um.medal_id = m.id AND um.user_id = $1 AND um.wearing AND (um.expires_at IS NULL OR um.expires_at > now()))) AS wearing          FROM medals m LEFT JOIN medal_categories c ON c.id = m.category_id ORDER BY m.category_id, m.id",
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
        "SELECT EXISTS(SELECT 1 FROM user_medals WHERE user_id = $1 AND medal_id = $2 AND (expires_at IS NULL OR expires_at > now()))",
    )
    .bind(auth.id)
    .bind(body.medal_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if owned {
        return Err(DomainError::Validation("已拥有该勋章".into()));
    }
    // 限量与销售期校验（medals.inventory NULL = 不限量；窗口 NULL = 长期在售）
    let (inventory, inv_used, sale_begin, sale_end): (
        Option<i32>,
        i64,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        "SELECT m.inventory, (SELECT count(*) FROM user_medals um WHERE um.medal_id = m.id), \
                m.sale_begin_at, m.sale_end_at \
         FROM medals m WHERE m.id = $1",
    )
    .bind(body.medal_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(begin) = sale_begin {
        if chrono::Utc::now() < begin {
            return Err(DomainError::Validation("该勋章尚未开售".into()));
        }
    }
    if let Some(end) = sale_end {
        if chrono::Utc::now() > end {
            return Err(DomainError::Validation("该勋章已结束销售".into()));
        }
    }
    if let Some(stock) = inventory {
        if inv_used >= stock as i64 {
            return Err(DomainError::Validation("该勋章已售罄".into()));
        }
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
    // 限时勋章按 duration_days 写 expires_at（0067；NULL = 永久）
    sqlx::query(
        "INSERT INTO user_medals (user_id, medal_id, source, expires_at) \
         SELECT $1, $2, 'buy', now() + make_interval(days => m.duration_days) \
         FROM medals m WHERE m.id = $2 \
         ON CONFLICT (user_id, medal_id) DO NOTHING",
    )
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
    sqlx::query(
        "INSERT INTO user_medals (user_id, medal_id, source, expires_at) \
         SELECT $1, $2, 'gift', now() + make_interval(days => m.duration_days) \
         FROM medals m WHERE m.id = $2 \
         ON CONFLICT (user_id, medal_id) DO NOTHING",
    )
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
    // 单佩戴位：先全部摘下再佩戴指定勋章。两条语句需原子完成——否则「佩戴未拥有勋章」
    // 报错回滚时会把原本已佩戴的勋章也摘掉（先摘后戴的非事务写已生效，无法随错误回退）。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("UPDATE user_medals SET wearing = false WHERE user_id = $1")
        .bind(auth.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(mid) = body.medal_id {
        let updated = sqlx::query(
            "UPDATE user_medals SET wearing = true WHERE user_id = $1 AND medal_id = $2",
        )
        .bind(auth.id)
        .bind(mid)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if updated.rows_affected() == 0 {
            // 回滚：保留佩戴原状（未拥有该勋章时不动已佩戴勋章）
            tx.rollback()
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            return Err(DomainError::Validation("未拥有该勋章".into()));
        }
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "wearing": body.medal_id })))
}

#[get("/me/medals")]
async fn my_medals(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<(i64, String, bool)> = sqlx::query_as(
        "SELECT m.id, m.name, um.wearing FROM user_medals um JOIN medals m ON m.id = um.medal_id WHERE um.user_id = $1 AND (um.expires_at IS NULL OR um.expires_at > now()) ORDER BY m.id",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ M15 论坛（forums.php 三层权限口径） ============
//
// 三层判定叠加：
//   1) 版块三档门槛：minclassread 看 / minclasswrite 回 / minclasscreate 发主题（逐版块独立）
//   2) 版主：forum_mods（forumid+userid，无需等级，仅本版块有效）
//   3) 全局：postmanage = class≥90 全版块管帖；forummanage（≥93，见 admin_http）管版块
// 反直觉点照搬：作者不能删自己的帖（删帖只对版主/postmanage 开放）；
// 删主题收回发帖 +2 火花；管理员编辑他人帖自动 PM 通知并留 edited_by；
// forumpost=FALSE 账户级禁言；发帖 10 秒防刷（postmanage 豁免）；
// 受保护版块 2 楼起正文替换为提示（class≥90/发帖人本人/楼主/本版版主放行）。

/// 三层权限判定结果
#[derive(serde::Serialize, sqlx::FromRow, Clone, Copy)]
struct ForumPerm {
    can_read: bool,
    can_write: bool,
    can_create: bool,
    can_mod: bool,
}

async fn forum_access(
    db: &sqlx::PgPool,
    user_id: i64,
    class_id: i32,
    forum_id: i64,
) -> DomainResult<ForumPerm> {
    let none = ForumPerm {
        can_read: false,
        can_write: false,
        can_create: false,
        can_mod: false,
    };
    let row: Option<(i32, i32, i32)> = sqlx::query_as(
        "SELECT minclassread, minclasswrite, minclasscreate FROM forums WHERE id = $1",
    )
    .bind(forum_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((min_read, min_write, min_create)) = row else {
        return Ok(none);
    };
    // 全局 postmanage：全版块放行
    if class_id >= 90 {
        return Ok(ForumPerm {
            can_read: true,
            can_write: true,
            can_create: true,
            can_mod: true,
        });
    }
    // 版主：本版块全放行（任命不需要等级）
    let is_mod: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM forum_mods WHERE forum_id = $1 AND user_id = $2)",
    )
    .bind(forum_id)
    .bind(user_id)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 账户级禁言（forumpost='no' 口径）：与等级、版块门槛无关，一律不能发帖回帖
    let can_post: bool =
        sqlx::query_scalar("SELECT COALESCE(forumpost, TRUE) FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(db)
            .await
            .unwrap_or(true);
    let can_read = is_mod || class_id >= min_read;
    let can_write = can_read && can_post && (is_mod || class_id >= min_write);
    let can_create = can_write && (is_mod || class_id >= min_create);
    Ok(ForumPerm {
        can_read,
        can_write,
        can_create,
        can_mod: is_mod,
    })
}

/// 发帖 10 秒防刷（postmanage/版主豁免）
async fn forum_flood_check(db: &sqlx::PgPool, user_id: i64, class_id: i32) -> DomainResult<()> {
    if class_id >= 90 {
        return Ok(());
    }
    let last: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT last_sent_at FROM forum_flood WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
    if let Some(t) = last {
        if (chrono::Utc::now() - t).num_seconds() < 10 {
            return Err(DomainError::Validation(
                "发送过于频繁，请 10 秒后再试".into(),
            ));
        }
    }
    Ok(())
}

async fn forum_flood_mark(db: &sqlx::PgPool, user_id: i64) {
    let _ = sqlx::query(
        "INSERT INTO forum_flood (user_id, last_sent_at) VALUES ($1, now()) \
         ON CONFLICT (user_id) DO UPDATE SET last_sent_at = now()",
    )
    .bind(user_id)
    .execute(db)
    .await;
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct ForumRow {
    id: i64,
    name: String,
    descr: Option<String>,
    topics: i64,
    posts: i64,
    #[sqlx(default)]
    latest_topic: Option<String>,
    #[sqlx(default)]
    latest_author: Option<String>,
    #[sqlx(default)]
    latest_at: Option<chrono::DateTime<chrono::Utc>>,
    can_write: bool,
    can_create: bool,
    can_mod: bool,
}

#[get("/forums")]
async fn forum_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 只列本版块门槛允许（minclassread）+ 版主兼任的版块
    let rows = sqlx::query_as::<_, (i64, String, Option<String>, i64, i64)>(
        "SELECT f.id, f.name, f.descr, \
            (SELECT count(*) FROM topics t WHERE t.forum_id = f.id) AS topics, \
            (SELECT count(*) FROM posts p JOIN topics t ON t.id = p.topic_id WHERE t.forum_id = f.id) AS posts \
         FROM forums f \
         WHERE f.minclassread <= $1 OR EXISTS (SELECT 1 FROM forum_mods fm WHERE fm.forum_id = f.id AND fm.user_id = $2) \
         ORDER BY f.id",
    )
    .bind(auth.class_id)
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let mut latest: std::collections::HashMap<
        i64,
        (
            Option<String>,
            Option<String>,
            Option<chrono::DateTime<chrono::Utc>>,
        ),
    > = std::collections::HashMap::new();
    let latest_rows: Vec<(
        i64,
        Option<String>,
        Option<String>,
        Option<chrono::DateTime<chrono::Utc>>,
    )> = sqlx::query_as(
        "SELECT DISTINCT ON (t.forum_id) t.forum_id, t.title, u.username, t.created_at \
         FROM topics t LEFT JOIN users u ON u.id = t.user_id \
         WHERE t.forum_id = ANY(SELECT id FROM forums WHERE minclassread <= $1) \
         ORDER BY t.forum_id, t.id DESC",
    )
    .bind(auth.class_id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    for (fid, title, author, at) in latest_rows {
        latest.insert(fid, (title, author, at));
    }

    let mut out = Vec::with_capacity(rows.len());
    for (id, name, descr, topics, posts) in rows {
        let perm = forum_access(&state.repo.db, auth.id, auth.class_id, id).await?;
        let (latest_topic, latest_author, latest_at) =
            latest.get(&id).cloned().unwrap_or((None, None, None));
        out.push(ForumRow {
            id,
            name,
            descr,
            topics,
            posts,
            latest_topic,
            latest_author,
            latest_at,
            can_write: perm.can_write,
            can_create: perm.can_create,
            can_mod: perm.can_mod,
        });
    }
    Ok(ok(out))
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
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, body.forum_id).await?;
    if !perm.can_create {
        return Err(DomainError::Forbidden);
    }
    forum_flood_check(&state.repo.db, auth.id, auth.class_id).await?;
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
    forum_flood_mark(&state.repo.db, auth.id).await;
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
    sticky: bool,
    locked: bool,
}

#[get("/forums/{id}/topics")]
async fn topic_list(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let fid = path.into_inner();
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_read {
        return Err(DomainError::Forbidden);
    }
    let rows = sqlx::query_as::<_, TopicRow>(
        "SELECT t.id, t.forum_id, t.title, u.username, \
            (SELECT count(*)-1 FROM posts p WHERE p.topic_id = t.id) AS replies, t.views, t.last_post_at, \
            t.sticky, t.locked \
         FROM topics t LEFT JOIN users u ON u.id = t.user_id \
         WHERE t.forum_id = $1 ORDER BY t.sticky DESC, t.id DESC LIMIT 50",
    )
    .bind(fid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "forum_id": fid,
        "can_write": perm.can_write,
        "can_create": perm.can_create,
        "can_mod": perm.can_mod,
        "topics": rows,
    })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct PostRow {
    id: i64,
    username: Option<String>,
    user_id: Option<i64>,
    body: String,
    created_at: chrono::DateTime<chrono::Utc>,
    edited_at: Option<chrono::DateTime<chrono::Utc>>,
    edited_by: Option<i64>,
    #[serde(skip)]
    hidden: bool,
}

#[get("/forums/topics/{id}")]
async fn topic_detail(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    sqlx::query("UPDATE topics SET views = views + 1 WHERE id = $1")
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // NexusPHP 帖子页头部：主题标题 + 所属版块（找不到主题时 404）
    let meta: Option<(String, i64, Option<String>, Option<i64>, bool, bool)> = sqlx::query_as(
        "SELECT t.title, f.id, f.name, t.user_id, t.sticky, t.locked \
         FROM topics t LEFT JOIN forums f ON f.id = t.forum_id WHERE t.id = $1",
    )
    .bind(tid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((title, fid, forum_name, op_id, sticky, locked)) = meta else {
        return Err(DomainError::NotFound(tid));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_read {
        return Err(DomainError::Forbidden);
    }
    let mut posts = sqlx::query_as::<_, PostRow>(
        "SELECT p.id, u.username, p.user_id, p.body, p.created_at, p.edited_at, p.edited_by, FALSE AS hidden \
         FROM posts p LEFT JOIN users u ON u.id = p.user_id \
         WHERE p.topic_id = $1 ORDER BY p.id LIMIT 200",
    )
    .bind(tid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 受保护版块：2 楼起正文替换为提示（class≥90 / 发帖人本人 / 楼主 / 本版版主放行）
    let protected: bool = sqlx::query_scalar("SELECT protected FROM forums WHERE id = $1")
        .bind(fid)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
    if protected {
        let is_op = op_id == Some(auth.id);
        for (i, p) in posts.iter_mut().enumerate() {
            let is_self = p.user_id == Some(auth.id);
            if i > 0 && !is_self && !is_op && !perm.can_mod {
                p.hidden = true;
                p.body = "……".to_string();
            }
        }
    }
    Ok(ok(serde_json::json!({
        "topic_id": tid,
        "title": title,
        "forum_id": fid,
        "forum_name": forum_name,
        "sticky": sticky,
        "locked": locked,
        "is_op": op_id == Some(auth.id),
        "current_user_id": auth.id,
        "can_write": perm.can_write,
        "can_mod": perm.can_mod,
        "posts": posts,
    })))
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
    let row: Option<(i64, bool)> =
        sqlx::query_as("SELECT forum_id, locked FROM topics WHERE id = $1")
            .bind(tid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((fid, locked)) = row else {
        return Err(DomainError::NotFound(tid));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_write {
        return Err(DomainError::Forbidden);
    }
    if locked && !perm.can_mod {
        return Err(DomainError::Validation("主题已锁定".into()));
    }
    forum_flood_check(&state.repo.db, auth.id, auth.class_id).await?;
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
    forum_flood_mark(&state.repo.db, auth.id).await;
    // 回帖 +1 火花
    let idem = format!("forum-reply:{}:{}", auth.id, post_id);
    earn_spark(&state.repo.db, auth.id, 1, "forum", &idem).await?;
    Ok(ok(serde_json::json!({ "post_id": post_id })))
}

// ---- 论坛管理操作（版主限本版块 / postmanage 全站） ----

/// 单帖上下文：(topic_id, forum_id, author_id)
async fn post_context(db: &sqlx::PgPool, post_id: i64) -> DomainResult<Option<(i64, i64, i64)>> {
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT p.topic_id, t.forum_id, p.user_id FROM posts p \
         JOIN topics t ON t.id = p.topic_id \
         WHERE p.id = $1",
    )
    .bind(post_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(row)
}

/// 编辑帖子：作者本人随时可改；版主/postmanage 可改他人帖 —— 后者自动 PM 通知作者并留 edited_by
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
    let pid = path.into_inner();
    let Some((_tid, fid, author_id)) = post_context(&state.repo.db, pid).await? else {
        return Err(DomainError::NotFound(pid));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_mod && author_id != auth.id {
        return Err(DomainError::Forbidden);
    }
    let n =
        sqlx::query("UPDATE posts SET body = $1, edited_at = now(), edited_by = $2 WHERE id = $3")
            .bind(body.body.trim())
            .bind(auth.id)
            .bind(pid)
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
    let Some((tid, fid, _author)) = post_context(&state.repo.db, pid).await? else {
        return Err(DomainError::NotFound(pid));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_mod {
        return Err(DomainError::Forbidden);
    }
    sqlx::query("DELETE FROM posts WHERE id = $1 AND topic_id = $2")
        .bind(pid)
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
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
    let fid: Option<i64> = sqlx::query_scalar("SELECT forum_id FROM topics WHERE id = $1")
        .bind(tid)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(fid) = fid else {
        return Err(DomainError::NotFound(tid));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_mod {
        return Err(DomainError::Forbidden);
    }
    let op: Option<i64> = sqlx::query_scalar("SELECT user_id FROM topics WHERE id = $1")
        .bind(tid)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
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
    sqlx::query("DELETE FROM topics WHERE id = $1")
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "forum.topic_delete", Some(tid))
        .await;
    Ok(ok(serde_json::json!({ "deleted": tid })))
}

/// 主题管理：置顶/锁定/移动版块（版主限本版块）
#[derive(Deserialize)]
struct TopicManageReq {
    #[serde(default)]
    sticky: Option<bool>,
    #[serde(default)]
    locked: Option<bool>,
    #[serde(default)]
    move_to_forum_id: Option<i64>,
}

#[post("/forums/topics/{id}/manage")]
async fn topic_manage(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TopicManageReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    let fid: Option<i64> = sqlx::query_scalar("SELECT forum_id FROM topics WHERE id = $1")
        .bind(tid)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(fid) = fid else {
        return Err(DomainError::NotFound(tid));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_mod {
        return Err(DomainError::Forbidden);
    }
    let mut moved = false;
    if let Some(target) = body.move_to_forum_id {
        if target != fid {
            let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM forums WHERE id = $1")
                .bind(target)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            if exists.is_none() {
                return Err(DomainError::Validation("目标版块不存在".into()));
            }
            sqlx::query("UPDATE topics SET forum_id = $1 WHERE id = $2")
                .bind(target)
                .bind(tid)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            moved = true;
        }
    }
    if let Some(sticky) = body.sticky {
        sqlx::query("UPDATE topics SET sticky = $1 WHERE id = $2")
            .bind(sticky)
            .bind(tid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    if let Some(locked) = body.locked {
        sqlx::query("UPDATE topics SET locked = $1 WHERE id = $2")
            .bind(locked)
            .bind(tid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    state
        .repo
        .audit(Some(auth.id), "forum.topic_manage", Some(tid))
        .await;
    Ok(ok(serde_json::json!({ "managed": tid, "moved": moved })))
}
// ============ M16 短讯与好友 ============

// （我的 H&R /me/hr 由 gaps_http::my_hr_status 提供——hr_snapshots 口径；
//  旧 snatches 口径实现已删除，曾与前者重复注册同一路由）

// ============ 聊天盒（shoutbox.php 口径：最近消息 + 发言） ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct ShoutRow {
    id: i64,
    username: Option<String>,
    message: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/shoutbox")]
async fn shoutbox_list(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<HttpResponse> {
    let rows: Vec<ShoutRow> = sqlx::query_as(
        "SELECT sb.id, u.username, sb.message, sb.created_at          FROM shoutbox sb LEFT JOIN users u ON u.id = sb.user_id          ORDER BY sb.id DESC LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct ShoutReq {
    message: String,
}

#[post("/shoutbox")]
async fn shoutbox_send(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ShoutReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.message.trim().is_empty() || body.message.len() > 300 {
        return Err(DomainError::Validation("发言需 1-300 字".into()));
    }
    let id: i64 =
        sqlx::query_scalar("INSERT INTO shoutbox (user_id, message) VALUES ($1, $2) RETURNING id")
            .bind(auth.id)
            .bind(body.message.trim())
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(Deserialize)]
struct SendMsgReq {
    to: String,
    subject: String,
    body: String,
    /// 回复/转发的原信 id（回复自动加 Re: 前缀+引用原文；转发原样带文）
    #[serde(default)]
    reply_to: Option<i64>,
    #[serde(default)]
    forward_of: Option<i64>,
}

/// 发信（sendmessage.php 口径 + takemessage 的管理组豁免）：
/// - 接收限制：普通用户须过对方 accept_pm（yes/friends/no）+ 非黑名单；管理组（staff≥90）一律放行
/// - 防刷：普通用户 60s 一条（message_flood 表）；staff 不限
/// - 回复：subject 加 Re:/Re(n): 并引用原文；转发：原封带文给第三人
#[post("/messages")]
async fn message_send(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SendMsgReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let staff = auth.class_id >= 90;

    let target: Option<(i64, String, i32)> = sqlx::query_as(
        "SELECT id, accept_pm, class_id FROM users WHERE username = $1 AND status < 2",
    )
    .bind(&body.to)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((to_id, accept, _to_class)) = target else {
        return Err(DomainError::NotFound(0));
    };
    if body.subject.trim().is_empty() {
        return Err(DomainError::Validation("主题不能为空".into()));
    }
    // 接收限制（管理组豁免 —— takemessage.php staffmem 口径）
    if !staff {
        match accept.as_str() {
            "no" => return Err(DomainError::Validation("对方仅接收管理组私信".into())),
            "friends" => {
                let is_friend: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM friendships f WHERE (f.user_id=$1 AND f.friend_id=$2) OR (f.user_id=$2 AND f.friend_id=$1))",
                )
                .bind(auth.id)
                .bind(to_id)
                .fetch_one(&state.repo.db)
                .await
                .unwrap_or(false);
                if !is_friend {
                    return Err(DomainError::Validation("对方仅接收好友私信".into()));
                }
            }
            _ => {}
        }
        // 防刷：60s 一条
        let last: Option<chrono::DateTime<chrono::Utc>> =
            sqlx::query_scalar("SELECT last_sent_at FROM message_flood WHERE user_id = $1")
                .bind(auth.id)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .flatten();
        if let Some(t) = last {
            if (chrono::Utc::now() - t).num_seconds() < 60 {
                return Err(DomainError::Validation(
                    "发送过于频繁，请 1 分钟后再试".into(),
                ));
            }
        }
    }

    // 回复：Re:/Re(n): 前缀 + 引用原文；转发：原样带文
    let mut subject = body.subject.trim().to_string();
    let mut text = body.body.clone();
    if let Some(rid) = body.reply_to {
        let orig: Option<(String, String)> = sqlx::query_as(
            "SELECT subject, body FROM messages WHERE id = $1 AND (receiver_id = $2 OR sender_id = $2)",
        )
        .bind(rid)
        .bind(auth.id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if let Some((osub, obody)) = orig {
            let n = osub.match_indices("Re(").count();
            subject = if n == 0 && osub.starts_with("Re:") {
                format!("Re: {}", osub.trim_start_matches("Re: "))
            } else if n > 0 {
                format!("Re({}): {}", n + 1, osub)
            } else {
                format!("Re: {}", osub)
            };
            text = format!("{}\n\n———— 原信 ————\n{}", body.body, obody);
        }
    }
    if let Some(fid) = body.forward_of {
        let orig: Option<(String, String)> = sqlx::query_as(
            "SELECT subject, body FROM messages WHERE id = $1 AND (receiver_id = $2 OR sender_id = $2)",
        )
        .bind(fid)
        .bind(auth.id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if let Some((osub, obody)) = orig {
            subject = format!("Fw: {}", osub);
            text = obody;
        }
    }

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO messages (sender_id, receiver_id, subject, body, location, saved, unread) \
         VALUES ($1, $2, $3, $4, 1, 1, true) RETURNING id",
    )
    .bind(auth.id)
    .bind(to_id)
    .bind(&subject)
    .bind(&text)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO message_flood (user_id, last_sent_at) VALUES ($1, now()) \
                 ON CONFLICT (user_id) DO UPDATE SET last_sent_at = now()",
    )
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .ok();
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
    #[sqlx(default)]
    unread: Option<bool>,
    #[sqlx(default)]
    folder: Option<i32>,
}

/// 收件箱（messages.php location=1 口径）：支持 box=folderid、关键词搜索（主题/正文/两者）与未读筛选
#[get("/messages/inbox")]
async fn message_inbox(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<InboxQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 关键词经 like_pattern 转义（%/_/\）后与原始串绑定：NULL 跳过、非 NULL 模糊匹配
    let pattern = q.search.as_deref().map(crate::http::like_pattern);
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT m.id, u.username AS counterpart, m.subject, m.body, m.read_at, m.created_at, m.unread, m.folder \
         FROM messages m LEFT JOIN users u ON u.id = m.sender_id \
         WHERE m.receiver_id = $1 AND m.location = 1 \
           AND ($2::int IS NULL OR m.folder = $2) \
           AND ($3::text IS NULL OR m.subject ILIKE $3 OR m.body ILIKE $3) \
           AND ($4::bool IS NULL OR m.unread = $4) \
         ORDER BY m.id DESC LIMIT 100",
    )
    .bind(auth.id)
    .bind(q.box_id)
    .bind(pattern.as_deref())
    .bind(q.unread)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct InboxQuery {
    /// 自建文件夹 id（缺省 = 主收件箱 folder IS NULL）
    box_id: Option<i32>,
    search: Option<String>,
    unread: Option<bool>,
}

/// 发件箱（saved=1 口径）
#[get("/messages/sent")]
async fn message_sent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT m.id, u.username AS counterpart, m.subject, m.body, m.read_at, m.created_at, m.unread, m.folder \
         FROM messages m LEFT JOIN users u ON u.id = m.receiver_id \
         WHERE m.sender_id = $1 AND m.saved = 1 ORDER BY m.id DESC LIMIT 100",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 管理组信箱（staffbox.php 口径）：与管理组成员（class_id >= 90）互发的短讯
#[get("/messages/staff")]
async fn message_staff(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT m.id, u.username AS counterpart, m.subject, m.body, m.read_at, m.created_at \
         FROM messages m \
         LEFT JOIN users u ON u.id = CASE WHEN m.sender_id = $1 THEN m.receiver_id ELSE m.sender_id END \
         WHERE (m.sender_id = $1 AND EXISTS (SELECT 1 FROM users su WHERE su.id = m.receiver_id AND su.class_id >= 90)) \
            OR (m.receiver_id = $1 AND EXISTS (SELECT 1 FROM users su WHERE su.id = m.sender_id AND su.class_id >= 90)) \
         ORDER BY m.id DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ 信箱操作（messages.php 口径：已读/逻辑删/移动/文件夹） ============

#[derive(Deserialize)]
struct MarkReadReq {
    ids: Vec<i64>,
}

/// 标记已读（单条打开或列表批量 markread）
#[post("/messages/markread")]
async fn message_markread(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MarkReadReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.ids.is_empty() {
        return Err(DomainError::Validation("未选择信件".into()));
    }
    let n = sqlx::query(
        "UPDATE messages SET unread = false, read_at = COALESCE(read_at, now()) \
         WHERE receiver_id = $1 AND id = ANY($2)",
    )
    .bind(auth.id)
    .bind(&body.ids)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    Ok(ok(serde_json::json!({ "updated": n })))
}

#[derive(Deserialize)]
struct DeleteReq {
    ids: Vec<i64>,
}

/// 逻辑删除（NexusPHP 双删语义）：收件方删 → location=0；发件方删 → saved=0；
/// 两边都删才物理 DELETE。
#[post("/messages/delete")]
async fn message_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DeleteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.ids.is_empty() {
        return Err(DomainError::Validation("未选择信件".into()));
    }
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE messages SET location = 0 WHERE receiver_id = $1 AND id = ANY($2) AND location = 1",
    )
    .bind(auth.id)
    .bind(&body.ids)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE messages SET saved = 0 WHERE sender_id = $1 AND id = ANY($2) AND saved = 1",
    )
    .bind(auth.id)
    .bind(&body.ids)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("DELETE FROM messages WHERE id = ANY($1) AND location = 0 AND saved = 0")
        .bind(&body.ids)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "deleted": body.ids.len() })))
}

#[derive(Deserialize)]
struct MoveReq {
    ids: Vec<i64>,
    folder: Option<i32>,
}

/// 移动到自建文件夹（folder=NULL 即移回主收件箱）
#[post("/messages/move")]
async fn message_move(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MoveReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.ids.is_empty() {
        return Err(DomainError::Validation("未选择信件".into()));
    }
    if let Some(f) = body.folder {
        let owned: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pmboxes WHERE id = $1 AND user_id = $2)",
        )
        .bind(f)
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        if !owned {
            return Err(DomainError::Validation("目标文件夹不存在".into()));
        }
    }
    let n = sqlx::query(
        "UPDATE messages SET folder = $2 WHERE receiver_id = $1 AND id = ANY($3) AND location = 1",
    )
    .bind(auth.id)
    .bind(body.folder)
    .bind(&body.ids)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    Ok(ok(serde_json::json!({ "moved": n })))
}

/// 文件夹列表 + 新建/改名（editmailboxes 口径：一人最多 3 个、名 ≤14 字；清空名=删除并连带清信）
#[derive(sqlx::FromRow, serde::Serialize)]
struct PmBoxRow {
    id: i64,
    name: String,
    count: i64,
}

#[get("/messages/boxes")]
async fn message_boxes(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, PmBoxRow>(
        "SELECT b.id, b.name, (SELECT count(*) FROM messages m WHERE m.folder = b.id AND m.receiver_id = $1 AND m.location = 1) AS count \
         FROM pmboxes b WHERE b.user_id = $1 ORDER BY b.id",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct BoxReq {
    /// 缺省 = 新建；有值 = 改名/删除该 id
    id: Option<i64>,
    name: String,
}

#[post("/messages/boxes")]
async fn message_box_upsert(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BoxReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let name = body.name.trim();
    // 清空名 = 删除文件夹（连带清掉夹内信件，NexusPHP 口径）
    if name.is_empty() {
        if let Some(bid) = body.id {
            sqlx::query("DELETE FROM messages WHERE folder = $2 AND receiver_id = $1")
                .bind(auth.id)
                .bind(bid)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query("DELETE FROM pmboxes WHERE id = $2 AND user_id = $1")
                .bind(auth.id)
                .bind(bid)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
        return Ok(ok(serde_json::json!({ "deleted": true })));
    }
    if name.chars().count() > 14 {
        return Err(DomainError::Validation("文件夹名最多 14 字".into()));
    }
    match body.id {
        None => {
            let n: i64 = sqlx::query_scalar("SELECT count(*) FROM pmboxes WHERE user_id = $1")
                .bind(auth.id)
                .fetch_one(&state.repo.db)
                .await
                .unwrap_or(0);
            if n >= 3 {
                return Err(DomainError::Validation("最多 3 个文件夹".into()));
            }
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO pmboxes (user_id, name) VALUES ($1, $2) \
                 ON CONFLICT (user_id, name) DO UPDATE SET name = EXCLUDED.name RETURNING id",
            )
            .bind(auth.id)
            .bind(name)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            Ok(ok(serde_json::json!({ "id": id })))
        }
        Some(bid) => {
            let n = sqlx::query("UPDATE pmboxes SET name = $3 WHERE id = $2 AND user_id = $1")
                .bind(auth.id)
                .bind(bid)
                .bind(name)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .rows_affected();
            if n == 0 {
                return Err(DomainError::NotFound(0));
            }
            Ok(ok(serde_json::json!({ "renamed": true })))
        }
    }
}

// ============ 咨询工作台（contactstaff → staffbox.php 口径） ============

#[derive(Deserialize)]
struct ContactReq {
    subject: String,
    body: String,
}

/// 用户提交咨询（takecontact.php 口径：写 staffmessages；普通用户 60s 防刷）
#[post("/contactstaff")]
async fn contact_staff(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ContactReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        let last: Option<chrono::DateTime<chrono::Utc>> =
            sqlx::query_scalar("SELECT last_sent_at FROM message_flood WHERE user_id = $1")
                .bind(auth.id)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .flatten();
        if let Some(t) = last {
            if (chrono::Utc::now() - t).num_seconds() < 60 {
                return Err(DomainError::Validation(
                    "发送过于频繁，请 1 分钟后再试".into(),
                ));
            }
        }
    }
    if body.subject.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("主题与正文不能为空".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO staffmessages (user_id, subject, body) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.subject.trim())
    .bind(body.body.trim())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO message_flood (user_id, last_sent_at) VALUES ($1, now()) \
                 ON CONFLICT (user_id) DO UPDATE SET last_sent_at = now()",
    )
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .ok();
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct StaffMessageRow {
    id: i64,
    username: Option<String>,
    subject: String,
    body: String,
    answered: i32,
    answered_by: Option<String>,
    answer: Option<String>,
    answered_at: Option<chrono::DateTime<chrono::Utc>>,
    permission: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// 咨询列表（staffbox.php）：全量给 staff（≥90）；permission 标签按标签名分流——
/// 与 NP 的权限位映射不同，这里简化为「全员可见普通咨询 + 带 permission 标签的定向分流
/// 也全员可见」，保留字段供未来按权限位过滤。
#[get("/staffmessages")]
async fn staff_messages(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<StaffMsgQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_MESSAGE).await?;
    let rows = sqlx::query_as::<_, StaffMessageRow>(
        "SELECT s.id, u.username, s.subject, s.body, s.answered, a.username AS answered_by, \
                s.answer, s.answered_at, s.permission, s.created_at \
         FROM staffmessages s \
         LEFT JOIN users u ON u.id = s.user_id \
         LEFT JOIN users a ON a.id = s.answered_by \
         WHERE ($1::int IS NULL OR s.answered = $1) \
         ORDER BY s.answered ASC, s.id DESC LIMIT 100",
    )
    .bind(q.answered)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct StaffMsgQuery {
    answered: Option<i32>,
}

#[derive(Deserialize)]
struct AnswerReq {
    id: i64,
    answer: String,
}

/// 答复（takeanswer 口径）：给来信人发一条私信 + 答复原文回写 staffmessages + 置已答复
#[post("/staffmessages/answer")]
async fn staff_answer(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AnswerReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_MESSAGE).await?;
    if body.answer.trim().is_empty() {
        return Err(DomainError::Validation("答复内容不能为空".into()));
    }
    let orig: Option<(i64, String)> =
        sqlx::query_as("SELECT user_id, subject FROM staffmessages WHERE id = $1 AND answered = 0")
            .bind(body.id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((uid, subject)) = orig else {
        return Err(DomainError::Validation("来信不存在或已答复".into()));
    };
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // ① 给用户发私信（staff 直发，不走接收限制/防刷）
    sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body, location, saved, unread) \
         VALUES ($1, $2, $3, $4, 1, 1, true)",
    )
    .bind(auth.id)
    .bind(uid)
    .bind(format!("Re: {}", subject))
    .bind(body.answer.trim())
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // ② 回写答复原文 + 状态
    sqlx::query(
        "UPDATE staffmessages SET answered = 1, answered_by = $2, answer = $3, answered_at = now() WHERE id = $1",
    )
    .bind(body.id)
    .bind(auth.id)
    .bind(body.answer.trim())
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "staffmsg_answer", Some(body.id))
        .await;
    Ok(ok(serde_json::json!({ "answered": true })))
}

#[derive(Deserialize)]
struct StaffMsgActionReq {
    ids: Vec<i64>,
}

/// 批量标记已答复
#[post("/staffmessages/mark")]
async fn staff_mark(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<StaffMsgActionReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_MESSAGE).await?;
    let n = sqlx::query(
        "UPDATE staffmessages SET answered = 1, answered_by = COALESCE(answered_by, $2), answered_at = COALESCE(answered_at, now()) \
         WHERE id = ANY($1) AND answered = 0",
    )
    .bind(&body.ids)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    Ok(ok(serde_json::json!({ "marked": n })))
}

/// 删除来信（单条/批量）
#[post("/staffmessages/delete")]
async fn staff_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<StaffMsgActionReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_MESSAGE).await?;
    let n = sqlx::query("DELETE FROM staffmessages WHERE id = ANY($1)")
        .bind(&body.ids)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    Ok(ok(serde_json::json!({ "deleted": n })))
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
    let scope = crate::admin_p3_http::mount_p3_tools(crate::admin_p2_http::mount_p2_tools(
        crate::admin_http::mount_admin(scope),
    ));
    let scope = crate::settings_http::mount_settings(scope);
    let scope = crate::push_http::mount_push(scope);
    let scope = crate::gaps_http::mount_gaps(scope);
    let scope = crate::rss_http::mount_rss(scope);
    let scope = crate::twofa_http::mount_twofa(scope);
    let scope = crate::compat_http::mount_compat(scope);
    cfg.service(crate::openapi_http::mount_openapi(scope));
}
