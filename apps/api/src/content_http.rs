//! M17 求种/候选/字幕 + M18 课本中心 + M20 排行榜 HTTP 接口。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::{earn_spark, spend_spark, spend_spark_tx};
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
        .service(subtitle_download)
        // M18 课本
        .service(textbook_list)
        .service(textbook_link)
        // M20 排行榜
        .service(top_boards)
        // 0101 用户自购置顶/限时免费
        .service(promo_plans)
        .service(promo_buy)
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
    // 悬赏即时冻结（从余额划走，应种交付时转移给应种人）。
    // 审计修复（P0 吞钱）：扣费与建单此前非原子——INSERT 失败（超长/瞬断）时扣款已提交、
    // 且幂等键带随机 UUID 不可追回。改为：先建单占坑 → 扣费失败连单回滚（同一事务语义）。
    let id: i64 = if body.bounty > 0 {
        let mut tx = state
            .repo
            .db
            .begin()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO requests (user_id, title, descr, bounty) VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(auth.id)
        .bind(&body.title)
        .bind(&body.descr)
        .bind(body.bounty)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        // spend_spark 自带事务（锁行→幂等→扣款→流水）；此处复用同一连接保证原子性
        let idem = format!("req-bounty:{}:{}", auth.id, id);
        spend_spark_tx(
            &mut tx,
            auth.id,
            body.bounty,
            "request_bounty",
            &idem,
            "request_bounty",
            id,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        id
    } else {
        sqlx::query_scalar(
            "INSERT INTO requests (user_id, title, descr, bounty) VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(auth.id)
        .bind(&body.title)
        .bind(&body.descr)
        .bind(body.bounty)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    };
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
    // 仅已过审种子可提名候选（审计修复：旧版可对待审/被拒/软删种子发起，
    // promote 会直接置 approval_status=1 + official_tag 绕过审核流）
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
    /// 真实文件的附件 sha256（前端先调 POST /attachments 上传拿到）。
    /// 审计修复（P1 空壳链路）：旧版 file_ref 是客户端任意字符串或后端伪造的
    /// s3:// UUID——下载只回 JSON 引用，全链路无文件本体。现统一走 attachments
    /// 存储（本地 savedirectory 卷 + sha256 内容寻址 + 配额），file_ref 记
    /// attach://<sha>；历史外部引用（http(s):// 链接）仍按原样展示。
    #[serde(default)]
    file_sha: Option<String>,
    /// 兼容字段：外部字幕站直链（http/https），与本地附件二选一
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
    let file_ref = if let Some(sha) = body
        .file_sha
        .as_deref()
        .map(str::trim)
        .filter(|s| s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit()))
    {
        // 校验附件归属：必须是本人在 attachments 上传过的文件（防冒用他人 sha）
        let owned: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM attachments WHERE sha256 = $1 AND user_id = $2)",
        )
        .bind(sha)
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        if !owned {
            return Err(DomainError::Validation(
                "附件未上传或不存在（请先通过上传接口提交字幕文件）".into(),
            ));
        }
        format!("attach://{sha}")
    } else if let Some(ext) = body
        .file_ref
        .as_deref()
        .map(str::trim)
        .filter(|s| s.starts_with("http://") || s.starts_with("https://"))
    {
        ext.to_string()
    } else {
        return Err(DomainError::Validation(
            "请上传字幕文件（或提供 http/https 直链）".into(),
        ));
    };
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

/// 字幕下载：计数 +1；本地附件（attach://sha）直接回文件字节（Content-Disposition
/// 按 title 命名 .txt/.ass/.srt 兜底），外部直链返回 JSON 引用由前端跳转。
/// 审计修复（P1 空壳链路）：旧版只回 file_ref JSON——下载按钮点开是引用文本而非文件。
#[get("/subtitles/{id}/download")]
async fn subtitle_download(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let sid = path.into_inner();
    let row: Option<(String, Option<i64>, String)> =
        sqlx::query_as("SELECT file_ref, torrent_id, title FROM subtitles WHERE id = $1")
            .bind(sid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((file_ref, torrent_id, title)) = row else {
        return Err(DomainError::NotFound(sid));
    };
    sqlx::query("UPDATE subtitles SET downloads = downloads + 1 WHERE id = $1")
        .bind(sid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(sha) = file_ref.strip_prefix("attach://") {
        // 与 /attachments/{sha} 同源读取（本地卷内容寻址），但不经 302：直接回字节，
        // 便于客户端「点开即存文件」；文件名用字幕标题（清洗非法字符）。
        let row: Option<(String, i64)> =
            sqlx::query_as("SELECT mime, size FROM attachments WHERE sha256 = $1")
                .bind(sha)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        let Some((mime, _size)) = row else {
            return Err(DomainError::NotFound(sid));
        };
        let bytes = crate::storage::get(&state.repo.db, sha)
            .await
            .ok_or(DomainError::NotFound(sid))?;
        let safe_title: String = title
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || " ._-()[]（）【】".contains(c) {
                    c
                } else {
                    '_'
                }
            })
            .take(80)
            .collect::<String>();
        let ext = if mime == "application/pdf" {
            "pdf"
        } else if mime == "text/plain" {
            "txt"
        } else {
            "bin"
        };
        return Ok(HttpResponse::Ok()
            .content_type(mime)
            .insert_header((
                actix_web::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{safe_title}.{ext}\""),
            ))
            .body(bytes));
    }
    let _ = auth;
    Ok(ok(serde_json::json!({
        "id": sid,
        "title": title,
        "torrent_id": torrent_id,
        "file_ref": file_ref,
    })))
}

/// 模块开关读取：module_{name} = 'no' 时模块关闭（site_type_packs 只是初始快照，
/// 运行时权威在 site_settings；此前后端不设防，仅前端隐藏导航，直连 URL 仍全功能可用）。
async fn module_disabled(db: &sqlx::PgPool, name: &str) -> bool {
    sqlx::query_scalar::<_, String>("SELECT value FROM site_settings WHERE name = $1")
        .bind(format!("module_{name}"))
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .map(|v| v == "no")
        .unwrap_or(false)
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
    if module_disabled(&state.repo.db, "textbooks").await {
        return Err(DomainError::NotFound(0)); // 模块已关闭：与前端导航隐藏同口径
    }
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
    if module_disabled(&state.repo.db, "textbooks").await {
        return Err(DomainError::NotFound(0)); // 模块已关闭
    }
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
    // 审计修复（P2）：seeding INNER JOIN 在无做种用户时全空导致恒空榜。
    // 改 LEFT JOIN + HAVING count>0：有做种记录者才进榜，口径与做种收益一致
    let hourly_q = base(
        "(10 + count(*) * 2 + COALESCE(sum(t.size), 0) / 1099511627776.0) \
            * CASE WHEN u.donor THEN 2 ELSE 1 END",
        "JOIN snatches s ON s.user_id = u.id AND s.seeding LEFT JOIN torrents t ON t.id = s.torrent_id",
        "",
        "HAVING count(*) > 0",
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

// ============ 用户自购置顶/限时免费（0101，好学站插件口径） ============

/// 价目/开关读取：promo_price.{kind}.{hours}（缺省价见迁移 0101；未定价的档位不可购买）
async fn promo_price(db: &sqlx::PgPool, kind: &str, hours: i32) -> Option<i64> {
    sqlx::query_scalar::<_, String>("SELECT value FROM site_settings WHERE name = $1")
        .bind(format!("promo_price.{kind}.{hours}"))
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|&p| p > 0)
}

#[derive(Deserialize)]
struct PromoBuyReq {
    torrent_id: i64,
    /// sticky1 一级置顶 | sticky2 二级置顶 | free 限时免费
    kind: String,
    /// 24 | 72（小时；与价目键对齐）
    hours: i32,
    #[serde(default)]
    idempotency_key: Option<String>,
}

/// 价目表（前端渲染档位；module_promo_buy=no 时返回 disabled）
#[get("/promo/plans")]
async fn promo_plans(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = require_auth(&req, &state).await?;
    let enabled = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'module_promo_buy'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .map(|v| v != "no")
    .unwrap_or(true);
    let mut plans = serde_json::Map::new();
    for kind in ["sticky1", "sticky2", "free"] {
        let mut entries = Vec::new();
        for hours in [24i32, 72i32] {
            if let Some(price) = promo_price(&state.repo.db, kind, hours).await {
                entries.push(serde_json::json!({ "hours": hours, "price": price }));
            }
        }
        plans.insert(kind.to_string(), entries.into());
    }
    Ok(ok(
        serde_json::json!({ "enabled": enabled, "plans": plans }),
    ))
}

/// 购买：本人种子或 staff 可购；扣费（spark_ledger 流水，幂等键含随机 nonce 由客户端
/// 提供——每次购买是独立消费行为，重试需带同一键）→ 置顶写 pos_state/pos_state_until
/// （延长语义：在现有效期内续购则顺延），免费写 promotions（torrent 专属 free）。
/// 生效校验在写入端完成；列表 sticky_expr 与促销裁决天然消费这些字段（零新查询）。
#[post("/promo/buy")]
async fn promo_buy(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PromoBuyReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if !["sticky1", "sticky2", "free"].contains(&body.kind.as_str()) {
        return Err(DomainError::Validation(
            "kind 需为 sticky1/sticky2/free".into(),
        ));
    }
    if ![24, 72].contains(&body.hours) {
        return Err(DomainError::Validation("hours 需为 24 或 72".into()));
    }
    let enabled = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'module_promo_buy'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .map(|v| v != "no")
    .unwrap_or(true);
    if !enabled {
        return Err(DomainError::Validation("本功能未开放".into()));
    }
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .ok_or(DomainError::Validation("缺少 idempotency_key".into()))?;

    let owner: Option<i64> =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1 AND approval_status = 1")
            .bind(body.torrent_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(body.torrent_id));
    };
    if owner != auth.id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let price = promo_price(&state.repo.db, &body.kind, body.hours)
        .await
        .ok_or_else(|| DomainError::Validation("该档位未定价，请联系站方".into()))?;

    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    spend_spark_tx(
        &mut tx,
        auth.id,
        price,
        "promo_buy",
        &idem,
        "torrent",
        body.torrent_id,
    )
    .await?;
    let ends: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT now() + make_interval(hours => $1)")
            .bind(body.hours)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    match body.kind.as_str() {
        // 置顶：GREATEST(现有到期, 新到期) 顺延；级别取本次购买档
        "sticky1" | "sticky2" => {
            let level = if body.kind == "sticky1" { 1i16 } else { 2i16 };
            sqlx::query(
                "UPDATE torrents SET \
                    pos_state = $2, \
                    pos_state_until = GREATEST(COALESCE(pos_state_until, now()), now()) + make_interval(hours => $3) \
                 WHERE id = $1",
            )
            .bind(body.torrent_id)
            .bind(level)
            .bind(body.hours)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // 限时免费：专属 torrent 促销（worker 计费裁决/列表角标同一数据源）
        "free" => {
            sqlx::query(
                "INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source, created_by) \
                 VALUES ('torrent', $1, 'free', now(), $2, 'manual', $3)",
            )
            .bind(body.torrent_id)
            .bind(ends)
            .bind(auth.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        _ => unreachable!(),
    }
    sqlx::query(
        "INSERT INTO promo_purchases (user_id, torrent_id, kind, hours, price, idempotency_key, ends_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (idempotency_key) DO NOTHING",
    )
    .bind(auth.id)
    .bind(body.torrent_id)
    .bind(&body.kind)
    .bind(body.hours)
    .bind(price)
    .bind(&idem)
    .bind(ends)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "promo_buy", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({
        "torrent_id": body.torrent_id, "kind": body.kind,
        "hours": body.hours, "price": price, "ends_at": ends,
    })))
}
