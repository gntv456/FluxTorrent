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
        .service(top_boards)
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
#[allow(dead_code)] // 列表响应复用字段，部分列暂未在 handler 中读取
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

/// 求种列表（包子站 viewrequests.php 口径）：finished 筛选 + 名称搜索。
/// 最新出价/评论数/应求数：数据模型暂无加价与评论表，取 bounty/0/0 兜底。
#[get("/requests")]
async fn request_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let finished = q.get("finished").map(|s| s.as_str()).unwrap_or("no");
    let search = q
        .get("search")
        .map(|s| crate::http::like_pattern(&s))
        .unwrap_or_else(|| "%".into());
    let status_cond = match finished {
        "all" => "TRUE",
        "yes" => "r.status = 1",
        "ing" => "r.status = 2",
        _ => "r.status = 0",
    };
    let only_mine = finished == "my";
    let rows: Vec<serde_json::Value> = if only_mine {
        sqlx::query_as::<_, (i64, Option<String>, String, Option<String>, i64, i16, Option<i64>, chrono::DateTime<chrono::Utc>)>(
            "SELECT r.id, u.username, r.title, r.descr, r.bounty, r.status, r.fulfilled_torrent_id, r.created_at \
             FROM requests r LEFT JOIN users u ON u.id = r.user_id \
             WHERE r.user_id = $2 AND r.title ILIKE $1 ORDER BY r.id DESC LIMIT 50",
        )
        .bind(&search)
        .bind(auth.id)
        .fetch_all(&state.repo.db)
        .await
    } else {
        sqlx::query_as::<_, (i64, Option<String>, String, Option<String>, i64, i16, Option<i64>, chrono::DateTime<chrono::Utc>)>(
            &format!(
                "SELECT r.id, u.username, r.title, r.descr, r.bounty, r.status, r.fulfilled_torrent_id, r.created_at \
                 FROM requests r LEFT JOIN users u ON u.id = r.user_id \
                 WHERE {status_cond} AND r.title ILIKE $1 ORDER BY r.id DESC LIMIT 50",
            ),
        )
        .bind(&search)
        .fetch_all(&state.repo.db)
        .await
    }
    .map_err(|e| DomainError::Internal(e.into()))?
    .into_iter()
    .map(|(id, username, title, descr, bounty, status, fulfilled, ts)| {
        serde_json::json!({
            "id": id, "username": username, "title": title, "descr": descr,
            "bounty": bounty, "latest_bounty": bounty,
            "comments": 0, "bids": 0,
            "status": status, "fulfilled_torrent_id": fulfilled, "created_at": ts,
        })
    })
    .collect();
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
    // 应种人必须是该种子的发布者（旧站口径）：防止他人拿别人的 torrent_id
    // 完结求种、把悬赏转入自己账户（原实现任何登录用户可领任意求种的 bounty）
    let owner_match: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1 AND owner_id = $2)")
            .bind(body.torrent_id)
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
    if !owner_match {
        return Err(DomainError::Validation(
            "只有该种子的发布者可以应种此求种".into(),
        ));
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
    // 悬赏：应种给他人 → 转移；自己应自己的 → 退还
    if bounty > 0 {
        let payee = if requester != auth.id {
            auth.id
        } else {
            requester
        };
        let idem = format!("req-payout:{}", id);
        earn_spark(&state.repo.db, payee, bounty, "task_reward", &idem).await?;
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
    let t_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1)")
        .bind(body.torrent_id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
    if !t_exists {
        return Err(DomainError::TorrentInvalid("种子不存在".into()));
    }
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
    // 前置校验存在性与转正态（原实现对不存在 offer 先占位→扣款触发 FK 500；
    // 对已转正 offer 扣款→回滚→404，报错语义混乱且浪费一轮账务）
    let offer_ok: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM offers WHERE id = $1 AND NOT promoted)")
            .bind(body.offer_id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
    if !offer_ok {
        return Err(DomainError::NotFound(body.offer_id));
    }
    // 先占位投票记录（原子判重，防重放刷票）
    let voted = sqlx::query("INSERT INTO offer_votes (offer_id, user_id, cost) VALUES ($1, $2, 1) ON CONFLICT DO NOTHING")
        .bind(body.offer_id)
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if voted.rows_affected() == 0 {
        return Err(DomainError::Validation("已投过票啦".into()));
    }
    // 候选投票 1 火花（旧站口径）；扣款失败回滚占位
    let idem = format!("offer-vote:{}:{}", auth.id, body.offer_id);
    if let Err(e) = spend_spark(
        &state.repo.db,
        auth.id,
        1,
        "vote",
        &idem,
        "offer",
        body.offer_id,
    )
    .await
    {
        let _ = sqlx::query("DELETE FROM offer_votes WHERE offer_id = $1 AND user_id = $2")
            .bind(body.offer_id)
            .bind(auth.id)
            .execute(&state.repo.db)
            .await;
        return Err(e);
    }
    let updated = sqlx::query("UPDATE offers SET votes = votes + 1 WHERE id = $1 AND NOT promoted")
        .bind(body.offer_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::NotFound(body.offer_id));
    }
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
    // 候选转正为管理操作
    crate::authz::require_perm(&state, &auth, crate::authz::perm::OFFERS_PROMOTE).await?;
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
    sqlx::query("UPDATE torrents SET approval_status = 1, official_tag = true, approved_at = now() WHERE id = $1")
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
    // torrent_id=0 或不存在的种子 → 存 NULL（外键可空），不阻断独立字幕分享
    let torrent_id = if body.torrent_id > 0 {
        Some(body.torrent_id)
    } else {
        None
    };
    let file_ref = body
        .file_ref
        .clone()
        .unwrap_or_else(|| format!("s3://subtitles/{}", Uuid::new_v4()));
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO subtitles (torrent_id, user_id, title, lang, file_ref) VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(torrent_id)
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
    /// 文件大小（字节；无真实文件时为 0）
    #[sqlx(default)]
    size: Option<i64>,
}

/// 字幕列表（包子站 subtitles.php 口径）：search 关键词 + lang 语言 + letter 首字母筛选
#[get("/subtitles")]
async fn subtitle_list(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<impl Responder> {
    let search = q
        .get("search")
        .map(|s| crate::http::like_pattern(&s))
        .unwrap_or_else(|| "%".into());
    let lang = q.get("lang_id").filter(|s| s.as_str() != "0").cloned();
    let letter = q.get("letter").filter(|s| !s.is_empty()).cloned();
    let rows = sqlx::query_as::<_, SubtitleRow>(
        "SELECT s.id, s.torrent_id, u.username, s.title, s.lang, s.downloads, s.created_at, 0::bigint AS size \
         FROM subtitles s LEFT JOIN users u ON u.id = s.user_id \
         WHERE s.title ILIKE $1 \
           AND ($2::text IS NULL OR s.lang = $2) \
           AND ($3::text IS NULL OR s.title ILIKE $3 || '%') \
         ORDER BY s.id DESC LIMIT 50",
    )
    .bind(search)
    .bind(lang)
    .bind(letter)
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

// ============ M20 排行榜（六榜卡片：最多魔力/上传量/下载量/最长做种时间/后宫时魔/发种量） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct TopRow {
    rank: i64,
    username: String,
    class_name: String,
    title: Option<String>,
    avatar_url: Option<String>,
    val: f64,
}

/// 六个榜单统一口径：status<2、每榜 Top10；后宫时魔 = 做种时魔（与 worker 小时结算同式：
/// 基础10 + 做种数×2 + 做种体积TB，捐赠者×2）
#[get("/top/boards")]
async fn top_boards(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let sel = "row_number() OVER (ORDER BY val DESC) AS rank, u.username, c.name AS class_name, \
        u.title, u.avatar_url, x.val::float8 AS val";
    let base = |agg: &str, joins: &str, extra_where: &str, group_by: &str| -> String {
        format!(
            "SELECT {sel} FROM ( \
                SELECT u.id AS uid, {agg} AS val \
                FROM users u {joins} \
                WHERE u.status < 2 {extra_where} \
                GROUP BY u.id {group_by} \
                ORDER BY val DESC LIMIT 10 \
            ) x JOIN users u ON u.id = x.uid JOIN user_classes c ON c.id = u.class_id"
        )
    };
    let bonus_q = base("u.spark_balance", "", "AND u.spark_balance > 0", "");
    let uploaded_q = base("u.uploaded", "", "AND u.uploaded > 0", "");
    let downloaded_q = base("u.downloaded", "", "AND u.downloaded > 0", "");
    let seedtime_q = base(
        "COALESCE(sum(s.seeded_seconds), 0) / 3600.0",
        "JOIN snatches s ON s.user_id = u.id",
        "",
        "",
    );
    let hourly_q = base(
        "(10 + count(*) * 2 + COALESCE(sum(t.size), 0) / 1099511627776.0) \
            * CASE WHEN u.donor THEN 2 ELSE 1 END",
        "JOIN snatches s ON s.user_id = u.id AND s.seeding JOIN torrents t ON t.id = s.torrent_id",
        "",
        "",
    );
    let torrents_q = base(
        "count(*)",
        "JOIN torrents t ON t.owner_id = u.id AND t.approval_status = 1",
        "",
        "",
    );

    let mut boards = serde_json::Map::new();
    for (key, q) in [
        ("bonus", bonus_q),
        ("uploaded", uploaded_q),
        ("downloaded", downloaded_q),
        ("seedtime", seedtime_q),
        ("hourly", hourly_q),
        ("torrents", torrents_q),
    ] {
        let rows = sqlx::query_as::<_, TopRow>(&q)
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        boards.insert(
            key.to_string(),
            serde_json::to_value(rows).unwrap_or_default(),
        );
    }
    Ok(ok(serde_json::Value::Object(boards)))
}
