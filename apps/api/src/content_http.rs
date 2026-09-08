//! M17 求种/候选/字幕 + M18 课本中心 + M20 排行榜 HTTP 接口。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use uuid::Uuid;

use crate::dto::ok;
use crate::economy_http::{earn_spark, spend_spark};
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

pub fn mount_content(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        // M17 求种
        .service(request_create)
        .service(request_list)
        .service(request_fulfill)
        // M17 候选
        .service(offer_create)
        .service(offer_list)
        .service(offer_vote)
        .service(offer_promote)
        // M17 字幕
        .service(subtitle_upload)
        .service(subtitle_list)
        // M18 课本
        .service(textbook_list)
        .service(textbook_link)
        // M20 排行榜
        .service(top_users)
}

// ============ M17 求种 ============

#[derive(Deserialize)]
struct RequestCreateReq {
    title: String,
    #[serde(default)]
    descr: Option<String>,
    #[serde(default)]
    bounty: i64,
}

#[post("/requests")]
async fn request_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RequestCreateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.title.trim().is_empty() {
        return Err(DomainError::Validation("求种标题不能为空".into()));
    }
    if body.bounty < 0 {
        return Err(DomainError::Validation("悬赏不能为负".into()));
    }
    // 悬赏即时冻结（从余额划走，应种交付时转移给应种人）
    if body.bounty > 0 {
        let idem = format!("req-bounty:{}:{}", auth.id, Uuid::new_v4());
        spend_spark(
            &state.repo.db,
            auth.id,
            body.bounty,
            "pool_donate",
            &idem,
            "request_bounty",
            0,
        )
        .await?;
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO requests (user_id, title, descr, bounty) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(auth.id)
    .bind(&body.title)
    .bind(&body.descr)
    .bind(body.bounty)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "id": id, "bounty_frozen": body.bounty }),
    ))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct RequestRow {
    id: i64,
    username: Option<String>,
    title: String,
    descr: Option<String>,
    bounty: i64,
    status: i16,
    fulfilled_torrent_id: Option<i64>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/requests")]
async fn request_list(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let rows = sqlx::query_as::<_, RequestRow>(
        "SELECT r.id, u.username, r.title, r.descr, r.bounty, r.status, r.fulfilled_torrent_id, r.created_at \
         FROM requests r LEFT JOIN users u ON u.id = r.user_id \
         WHERE r.status = 0 ORDER BY r.id DESC LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct FulfillReq {
    request_id: i64,
    torrent_id: i64,
}

#[post("/requests/fulfill")]
async fn request_fulfill(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FulfillReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let r: Option<(i64, i64, i64, i16)> =
        sqlx::query_as("SELECT id, user_id, bounty, status FROM requests WHERE id = $1")
            .bind(body.request_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((id, requester, bounty, status)) = r else {
        return Err(DomainError::NotFound(body.request_id));
    };
    if status != 0 {
        return Err(DomainError::Validation("该求种已处理".into()));
    }
    let t_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1 AND approval_status = 1)",
    )
    .bind(body.torrent_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !t_exists {
        return Err(DomainError::TorrentInvalid("种子不存在或未过审".into()));
    }
    let updated = sqlx::query(
        "UPDATE requests SET status = 1, fulfilled_torrent_id = $2 WHERE id = $1 AND status = 0",
    )
    .bind(id)
    .bind(body.torrent_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::LedgerConflict);
    }
    // 悬赏转移给应种人
    if bounty > 0 && requester != auth.id {
        let idem = format!("req-payout:{}", id);
        earn_spark(&state.repo.db, auth.id, bounty, "task_reward", &idem).await?;
    }
    Ok(ok(
        serde_json::json!({ "request_id": id, "bounty_paid": bounty }),
    ))
}

// ============ M17 候选 ============

#[derive(Deserialize)]
struct OfferCreateReq {
    torrent_id: i64,
}

#[post("/offers")]
async fn offer_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<OfferCreateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id: i64 =
        sqlx::query_scalar("INSERT INTO offers (user_id, torrent_id) VALUES ($1, $2) RETURNING id")
            .bind(auth.id)
            .bind(body.torrent_id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct OfferRow {
    id: i64,
    username: Option<String>,
    torrent_id: i64,
    torrent_name: Option<String>,
    votes: i32,
    promoted: bool,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/offers")]
async fn offer_list(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let rows = sqlx::query_as::<_, OfferRow>(
        "SELECT o.id, u.username, o.torrent_id, t.name AS torrent_name, o.votes, o.promoted, o.created_at \
         FROM offers o LEFT JOIN users u ON u.id = o.user_id \
         LEFT JOIN torrents t ON t.id = o.torrent_id \
         WHERE NOT o.promoted ORDER BY o.votes DESC, o.id DESC LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct OfferVoteReq {
    offer_id: i64,
}

#[post("/offers/vote")]
async fn offer_vote(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<OfferVoteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 候选投票 1 火花（旧站口径）
    let idem = format!("offer-vote:{}:{}", auth.id, body.offer_id);
    spend_spark(
        &state.repo.db,
        auth.id,
        1,
        "vote",
        &idem,
        "offer",
        body.offer_id,
    )
    .await?;
    let updated = sqlx::query("UPDATE offers SET votes = votes + 1 WHERE id = $1 AND NOT promoted")
        .bind(body.offer_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::NotFound(body.offer_id));
    }
    sqlx::query("INSERT INTO offer_votes (offer_id, user_id, cost) VALUES ($1, $2, 1) ON CONFLICT DO NOTHING")
        .bind(body.offer_id)
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "voted": true })))
}

#[derive(Deserialize)]
struct PromoteReq {
    offer_id: i64,
}

#[post("/offers/promote")]
async fn offer_promote(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PromoteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        return Err(DomainError::Forbidden); // 转正为管理操作
    }
    let tid: Option<i64> = sqlx::query_scalar(
        "UPDATE offers SET promoted = true WHERE id = $1 AND NOT promoted RETURNING torrent_id",
    )
    .bind(body.offer_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .flatten();
    let Some(tid) = tid else {
        return Err(DomainError::NotFound(body.offer_id));
    };
    sqlx::query("UPDATE torrents SET approval_status = 1, official_tag = true WHERE id = $1")
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "offer_promote", Some(tid))
        .await;
    Ok(ok(
        serde_json::json!({ "torrent_id": tid, "official": true }),
    ))
}

// ============ M17 字幕 ============

#[derive(Deserialize)]
struct SubtitleUploadReq {
    torrent_id: i64,
    title: String,
    #[serde(default)]
    lang: Option<String>,
    #[serde(default)]
    file_ref: Option<String>,
}

#[post("/subtitles")]
async fn subtitle_upload(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SubtitleUploadReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.title.trim().is_empty() {
        return Err(DomainError::Validation("字幕标题不能为空".into()));
    }
    let file_ref = body
        .file_ref
        .clone()
        .unwrap_or_else(|| format!("s3://subtitles/{}", Uuid::new_v4()));
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO subtitles (torrent_id, user_id, title, lang, file_ref) VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(body.torrent_id)
    .bind(auth.id)
    .bind(&body.title)
    .bind(&body.lang)
    .bind(&file_ref)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 发字幕 +5 火花（旧站口径）
    let idem = format!("subtitle:{}:{}", auth.id, id);
    earn_spark(&state.repo.db, auth.id, 5, "subtitle", &idem).await?;
    Ok(ok(serde_json::json!({ "id": id, "reward": 5 })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct SubtitleRow {
    id: i64,
    torrent_id: Option<i64>,
    username: Option<String>,
    title: String,
    lang: Option<String>,
    downloads: i32,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/subtitles")]
async fn subtitle_list(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let rows = sqlx::query_as::<_, SubtitleRow>(
        "SELECT s.id, s.torrent_id, u.username, s.title, s.lang, s.downloads, s.created_at \
         FROM subtitles s LEFT JOIN users u ON u.id = s.user_id \
         WHERE ($1::bigint IS NULL OR s.torrent_id = $1) ORDER BY s.id DESC LIMIT 50",
    )
    .bind(None::<i64>)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ M18 课本中心 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct TextbookRow {
    id: i64,
    subject: String,
    edition: String,
    grade: String,
    volume: Option<String>,
    publisher: Option<String>,
    downloads: i32,
    torrent_id: Option<i64>,
}

#[get("/textbooks")]
async fn textbook_list(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let rows = sqlx::query_as::<_, TextbookRow>(
        "SELECT tb.id, tb.subject, e.name AS edition, g.name AS grade, tb.volume, tb.publisher, tb.downloads, \
            (SELECT min(t.id) FROM torrents t WHERE t.textbook_id = tb.id AND t.approval_status = 1) AS torrent_id \
         FROM textbooks tb \
         JOIN editions e ON e.id = tb.edition_id \
         JOIN grades g ON g.id = tb.grade_id \
         ORDER BY tb.downloads DESC, tb.id LIMIT 100",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct TextbookLinkReq {
    torrent_id: i64,
    textbook_id: i64,
}

#[post("/textbooks/link")]
async fn textbook_link(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TextbookLinkReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let updated = sqlx::query("UPDATE torrents SET textbook_id = $2 WHERE id = $1")
        .bind(body.torrent_id)
        .bind(body.textbook_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::NotFound(body.torrent_id));
    }
    state
        .repo
        .audit(Some(auth.id), "textbook_link", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({ "linked": true })))
}

// ============ M20 排行榜 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct TopUserRow {
    rank: i64,
    username: String,
    class_name: String,
    uploaded: i64,
    downloaded: i64,
    seed_size: i64,
}

#[get("/top/users")]
async fn top_users(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    // 上传榜（聚合查询走索引；生产态物化视图 + L1 缓存 §M20）
    let rows = sqlx::query_as::<_, TopUserRow>(
        "SELECT row_number() OVER (ORDER BY u.uploaded DESC) AS rank, u.username, c.name AS class_name, \
            u.uploaded, u.downloaded, \
            COALESCE((SELECT sum(t.size) FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
              WHERE s.user_id = u.id AND s.seeding), 0)::bigint AS seed_size \
         FROM users u JOIN user_classes c ON c.id = u.class_id \
         WHERE u.status < 2 AND u.uploaded > 0 \
         ORDER BY u.uploaded DESC LIMIT 21",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}
