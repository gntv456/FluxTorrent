//! me 杂项与站点面：my_torrentlist/bookmarks/stats/cheat_events/rss_info。
//! 举报受理在 0285 拆到 `http::reports_http`。
//! 从 http.rs 按域拆出。

use crate::torrents;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

// auth 模块经 state.jwt 使用（0071 RS256 化后 http 层不再直接调用）

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::auth_infra::{require_auth, require_staff};

// ============ 我的做种/下载/完成列表（旧站 getusertorrentlist 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct SnatchRow {
    torrent_id: i64,
    name: String,
    size: i64,
    seeders: i32,
    leechers: i32,
    seeding: bool,
    leeching: bool,
    completed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(rename = "done")]
    uploaded_here: i64,
    /// 发布者视角的状态（仅 kind=uploads 填充）：0=待审 1=过审 2=被拒
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_status: Option<i16>,
    /// 被拒原因（approval_status=2 时给发布者自助整改）
    #[serde(skip_serializing_if = "Option::is_none")]
    deny_reason: Option<String>,
}

#[derive(Deserialize)]
struct SnatchQuery {
    /// seeding / leeching / completed / uploads
    kind: Option<String>,
    limit: Option<i64>,
}

#[get("/me/torrentlist")]
pub async fn my_torrentlist(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<SnatchQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let rows: Vec<SnatchRow> = match q.kind.as_deref() {
        // 自己的 uploads 放开全部审核态（0284 P0-1）：发种者要能跟踪「待审/被拒
        // + 拒因」，此前硬编码 =1 让待审种从「我的发布」消失。别人视角的
        // users/{id}/torrentlist 维持只看过审（user_torrentlist.rs）。
        Some("uploads") => sqlx::query_as(
            "SELECT t.id AS torrent_id, t.name, t.size, t.seeders, \
             t.leechers, false AS seeding, false AS leeching, \
             NULL::timestamptz AS completed_at, 0::bigint AS uploaded_here, \
             t.approval_status, \
             NULLIF(CONCAT_WS('：', dr.reason, \
               NULLIF(t.deny_note, '')), '') AS deny_reason \
             FROM torrents t \
             LEFT JOIN torrent_deny_reasons dr ON dr.id = t.deny_reason_id \
             WHERE t.owner_id = $1 \
             ORDER BY t.approval_status = 1 DESC, t.id DESC LIMIT $2",
        ),
        Some("completed") => sqlx::query_as(
            "SELECT s.torrent_id, t.name, t.size, t.seeders, t.leechers, s.seeding, s.leeching, \
             s.completed_at, s.uploaded AS uploaded_here \
             FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.completed_at IS NOT NULL \
             ORDER BY s.completed_at DESC LIMIT $2",
        ),
        // 默认做种中
        _ => sqlx::query_as(
            "SELECT s.torrent_id, t.name, t.size, t.seeders, t.leechers, s.seeding, s.leeching, \
             s.completed_at, s.uploaded AS uploaded_here \
             FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.seeding \
             ORDER BY s.torrent_id DESC LIMIT $2",
        ),
    }
    .bind(auth.id)
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 我的收藏列表（usercp 收藏夹口径）：bookmark 时间倒序
#[derive(Deserialize)]
struct BookmarksQuery {
    limit: Option<i64>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct BookmarkRow {
    torrent_id: i64,
    name: String,
    small_descr: Option<String>,
    size: i64,
    seeders: i32,
    leechers: i32,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/me/bookmarks")]
pub async fn my_bookmarks(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<BookmarksQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let rows: Vec<BookmarkRow> = sqlx::query_as(
        "SELECT t.id AS torrent_id, t.name, t.small_descr, t.size, t.seeders, t.leechers, \
         b.created_at FROM bookmarks b JOIN torrents t ON t.id = b.torrent_id \
         WHERE b.user_id = $1 AND t.approval_status = 1 \
         ORDER BY b.created_at DESC, t.id DESC LIMIT $2",
    )
    .bind(auth.id)
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ 统计（M10 / tracker 对接预演） ============

#[get("/stats")]
pub async fn stats(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    // 对象级读缓存（0069）：/stats 是全站聚合查询，首页/移动壳高频拉取。
    // TTL 60s 兜底（worker 每 60s 刷快照，口径一致）；Redis 故障直查库（fail-open 不阻断）。
    let key = "cache:stats:v1";
    let mut c = state.redis.clone();
    let hit: Option<String> =
        redis::AsyncCommands::get(&mut c, key).await.unwrap_or(None);
    if let Some(json) = hit {
        return Ok(ok(serde_json::from_str::<serde_json::Value>(&json)
            .unwrap_or(serde_json::Value::Null)));
    }
    let val = serde_json::to_value(torrents::site_stats(&state.repo.db).await?)
        .unwrap_or(serde_json::Value::Null);
    let _: Result<(), _> =
        redis::AsyncCommands::set_ex(&mut c, key, val.to_string(), 60u64).await;
    Ok(ok(val))
}

// ============ 作弊探测（0069 cheat_events 闭环查询端：tracker 拒绝 → worker 落库 → 后台可查） ============

#[derive(Deserialize)]
struct CheatEventsQuery {
    limit: Option<i64>,
}

/// agent_rules 黑白名单命中记录（staff 专用）：按最近命中倒序
#[get("/admin/cheat-events")]
pub async fn cheat_events_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<CheatEventsQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_staff(&auth)?;
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let rows: Vec<(
        i64,
        i64,
        String,
        Option<String>,
        String,
        i64,
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT id, user_id, agent, peer_ip, reason, hits, first_seen, last_seen \
             FROM cheat_events ORDER BY last_seen DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = rows
        .into_iter()
        .map(|(id, uid, agent, ip, reason, hits, first, last)| {
            serde_json::json!({
                "id": id, "user_id": uid, "agent": agent, "peer_ip": ip,
                "reason": reason, "hits": hits,
                "first_seen": first, "last_seen": last,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({ "items": items })))
}

#[get("/rss-info")]
pub async fn rss_info(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let passkey: String =
        sqlx::query_scalar("SELECT passkey FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    // 绝对地址口径与 rss feed 端点一致：PUBLIC_API_URL 优先，缺省回退请求 Host，
    // 不再硬编 127.0.0.1（生产未配该变量时用户复制的订阅地址必连失败）。
    let base = match std::env::var("PUBLIC_API_URL") {
        Ok(u) if !u.trim().is_empty() => {
            u.trim().trim_end_matches('/').to_string()
        }
        _ => {
            let host = req
                .headers()
                .get("host")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("127.0.0.1:8080");
            format!("http://{host}")
        }
    };
    Ok(ok(serde_json::json!({
        "urls": [
            { "label": "全部种子", "url": format!("{}/api/v1/rss/{}", base, passkey) },
            { "label": "官种", "url": format!("{}/api/v1/rss/{}?official=true", base, passkey) },
        ],
        "passkey": passkey,
        "base": format!("{}/api/v1/rss/", base),
    })))
}
