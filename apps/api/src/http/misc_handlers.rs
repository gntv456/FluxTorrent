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
    /// 发布者视角的状态（仅 kind=uploads 填充）：0=待审 1=过审 2=被拒。
    /// `#[sqlx(default)]`：这两列只有 /me/torrentlist 的 uploads 查询会投影，
    /// 公开资料的 `/users/{id}/torrentlist` 复用同一结构体却不投影——sqlx 对
    /// Option 字段只容忍 NULL、不容忍「列不存在」，于是只要那 6 个查询里任何
    /// 一条返回过行，整个资料页种子列表就 500。
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_status: Option<i16>,
    /// 被拒原因（approval_status=2 时给发布者自助整改）
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    deny_reason: Option<String>,
    /// 分类名（样图「我的发布」的分类列；仅 uploads 投影，其余臂为 None）
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    /// 发布时刻（uploads 用 `torrents.created_at`；`completed_at` 是
    /// 下载侧字段，发布者自己没有 snatch 行，旧查询因此一直回 NULL，
    /// 前端时间列恒显「—」）
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    published_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 一个 kind 的总数与（仅 uploads 有意义的）三档审核态计数。
#[derive(sqlx::FromRow, serde::Serialize)]
struct ListStats {
    total: i64,
    pending: i64,
    approved: i64,
    denied: i64,
}

#[derive(Deserialize)]
struct SnatchQuery {
    /// seeding / leeching / completed / uploads
    kind: Option<String>,
    /// 旧参数：等价于 per（保留一版，避免老客户端突然少看数据）
    limit: Option<i64>,
    #[serde(default)]
    per: Option<i64>,
    #[serde(default)]
    page: Option<i64>,
}

#[get("/me/torrentlist")]
pub async fn my_torrentlist(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<SnatchQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let per = q.per.or(q.limit).unwrap_or(50).clamp(1, 100);
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let kind = q.kind.as_deref().unwrap_or("seeding");
    let rows: Vec<SnatchRow> = match kind {
        // 自己的 uploads 放开全部审核态（0284 P0-1）：发种者要能跟踪「待审/被拒
        // + 拒因」，此前硬编码 =1 让待审种从「我的发布」消失。别人视角的
        // users/{id}/torrentlist 维持只看过审（user_torrentlist.rs）。
        "uploads" => sqlx::query_as(
            "SELECT t.id AS torrent_id, t.name, t.size, t.seeders, \
             t.leechers, false AS seeding, false AS leeching, \
             NULL::timestamptz AS completed_at, 0::bigint AS uploaded_here, \
             t.approval_status, t.created_at AS published_at, \
             c.name AS category, \
             NULLIF(CONCAT_WS('：', dr.reason, \
               NULLIF(t.deny_note, '')), '') AS deny_reason \
             FROM torrents t \
             LEFT JOIN torrent_deny_reasons dr ON dr.id = t.deny_reason_id \
             LEFT JOIN categories c ON c.id = t.category_id \
             WHERE t.owner_id = $1 \
             ORDER BY t.approval_status = 1 DESC, t.id DESC LIMIT $2 OFFSET $3",
        ),
        "completed" => sqlx::query_as(
            "SELECT s.torrent_id, t.name, t.size, t.seeders, t.leechers, s.seeding, s.leeching, \
             s.completed_at, s.uploaded AS uploaded_here \
             FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.completed_at IS NOT NULL \
             ORDER BY s.completed_at DESC LIMIT $2 OFFSET $3",
        ),
        // 默认做种中
        _ => sqlx::query_as(
            "SELECT s.torrent_id, t.name, t.size, t.seeders, t.leechers, s.seeding, s.leeching, \
             s.completed_at, s.uploaded AS uploaded_here \
             FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.seeding \
             ORDER BY s.torrent_id DESC LIMIT $2 OFFSET $3",
        ),
    }
    .bind(auth.id)
    .bind(per)
    .bind((page - 1) * per)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 计数与列表同一套 FROM/WHERE，否则前端按 total 画的页码里有空页。
    // uploads 顺手带三档审核态（样图的四格统计条），其余 kind 该三格为 0，
    // 由前端决定不渲染。
    let stats_sql: &str = if kind == "uploads" {
        "SELECT count(*) AS total, \
            count(*) FILTER (WHERE t.approval_status = 0) AS pending, \
            count(*) FILTER (WHERE t.approval_status = 1) AS approved, \
            count(*) FILTER (WHERE t.approval_status = 2) AS denied \
         FROM torrents t WHERE t.owner_id = $1"
    } else if kind == "completed" {
        "SELECT count(*) AS total, 0 AS pending, 0 AS approved, 0 AS denied \
         FROM snatches s WHERE s.user_id = $1 AND s.completed_at IS NOT NULL"
    } else {
        "SELECT count(*) AS total, 0 AS pending, 0 AS approved, 0 AS denied \
         FROM snatches s WHERE s.user_id = $1 AND s.seeding"
    };
    let lst: ListStats = sqlx::query_as::<_, ListStats>(stats_sql)
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "rows": rows,
        "total": lst.total,
        "page": page,
        "per": per,
        "stats": lst,
    })))
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
    q: crate::http::Q<BookmarksQuery>,
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
    // 0352：加挂用户有效 RSS token 列表（id/name/created_at——不回明文，
    // 明文只在创建/轮换响应里出现一次）。passkey 字段保留：旧 URL 永远兼容。
    let tokens: Vec<(i64, String, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT id, name, created_at FROM rss_tokens \
             WHERE user_id = $1 AND revoked_at IS NULL \
             ORDER BY created_at DESC, id DESC",
        )
        .bind(auth.id)
        .fetch_all(&state.repo.db)
        .await
        .unwrap_or_default();
    let token_items: Vec<serde_json::Value> = tokens
        .iter()
        .map(|(id, name, at)| {
            serde_json::json!({
                "id": id,
                "name": name,
                "created_at": at,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({
        "urls": [
            { "label": "全部种子", "url": format!("{}/api/v1/rss/{}", base, passkey) },
            { "label": "官种", "url": format!("{}/api/v1/rss/{}?official=true", base, passkey) },
        ],
        "passkey": passkey,
        "tokens": token_items,
        "base": format!("{}/api/v1/rss/", base),
    })))
}
