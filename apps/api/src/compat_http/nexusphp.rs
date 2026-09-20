//! 生态兼容层 ②：NexusPHP 字段口径的只读 JSON + download.php 形状下载。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::common::sha3_hex;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::openapi_http::require_token;
use crate::publish_http::build_torrent_bytes;
use crate::state::AppState;
use crate::torrents::{
    charge_for_download, list_torrents_noclamp, TorrentFilter,
};

// ============ NexusPHP 兼容端点（Token 鉴权，只读） ============

/// 当前 Token 所属用户（NP userdetails 口径：uploaded/downloaded/seedbonus/passkey）
#[get("/compat/nexusphp/user.json")]
async fn compat_np_user(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let (uid, _rpm) = require_token(&req, &state).await?;
    let row: Option<(i64, String, i64, i64, i64, i32, String)> =
        sqlx::query_as(
            "SELECT id, username, uploaded, downloaded, \
         COALESCE(spark_balance,0), class_id, passkey \
         FROM users WHERE id = $1 AND status < 2",
        )
        .bind(uid)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((
        id,
        username,
        uploaded,
        downloaded,
        seedbonus,
        class_id,
        passkey,
    )) = row
    else {
        return Err(DomainError::Unauthorized);
    };
    let ratio = if downloaded > 0 {
        (uploaded as f64 / downloaded as f64 * 10000.0).round() / 10000.0
    } else {
        -1.0
    };
    Ok(ok(serde_json::json!({
        "id": id,
        "username": username,
        "uploaded": uploaded,
        "downloaded": downloaded,
        "seedbonus": seedbonus,
        "class": class_id,
        "ratio": ratio,
        "passkey": passkey, // 本人 Token 才能读取；工具用它拼接 download.php 与 tracker 汇报
    })))
}

#[derive(Deserialize)]
struct NpListQuery {
    #[serde(default)]
    page: Option<i64>,
    #[serde(default)]
    pagesize: Option<i64>,
    #[serde(default)]
    keyword: Option<String>,
    #[serde(default)]
    category: Option<i32>,
    /// 搜索范围（NP 口径 0=标题 1=简介 3=发布者 4=IMDb）：主流工具（PT-Plugin-Plus 等）透传
    #[serde(default)]
    search_area: Option<i32>,
    /// 匹配模式（0=and 2=精确）
    #[serde(default)]
    search_mode: Option<i32>,
    /// IMDb 关键字（设置后等效 search_area=4）
    #[serde(default)]
    imdb: Option<String>,
}

/// 种子列表（NP torrents.php 字段口径，游标分页内部转 page 语义）
#[get("/compat/nexusphp/torrents.json")]
async fn compat_np_torrents(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<NpListQuery>,
) -> DomainResult<HttpResponse> {
    let (_uid, _rpm) = require_token(&req, &state).await?;
    let page = q.page.unwrap_or(1).max(1);
    let pagesize = q.pagesize.unwrap_or(30).clamp(1, 50);
    // IMDb 参数优先（NP 工具惯用 ?imdb=tt123 传法）；否则用显式 search_area
    let (search, search_area) = if let Some(im) =
        q.imdb.as_deref().map(str::trim).filter(|v| !v.is_empty())
    {
        (Some(im.to_string()), Some(4))
    } else {
        (
            q.keyword.clone().filter(|k| !k.trim().is_empty()),
            q.search_area,
        )
    };
    let filter = TorrentFilter {
        category_id: q.category.map(|v| vec![v]),
        search,
        search_area,
        search_mode: q.search_mode,
        ..TorrentFilter::default()
    };
    // 审计修复（P1）：list_torrents 内部把 limit 钳到 50，page*pagesize 在 page≥2 时
    // skip 后恒空（第 2 页起拿不到数据）。深翻页走 noclamp 版（调用方已 clamp pagesize≤50）。
    let p =
        list_torrents_noclamp(&state.repo.db, &filter, None, page * pagesize)
            .await?;
    let skip = ((page - 1) * pagesize) as usize;
    let items: Vec<_> = p
        .items
        .into_iter()
        .skip(skip)
        .take(pagesize as usize)
        .map(|t| {
            serde_json::json!({
                "id": t.id,
                "name": t.name,
                "small_descr": t.small_descr,
                "seeders": t.seeders,
                "leechers": t.leechers,
                "completed": t.times_completed,
                "size": t.size,
                "added": t.created_at.timestamp(),
                "category": t.category_id,
                "medium": t.medium_id,
                "promotion": t.promotion,
                "free": matches!(
                    t.promotion.as_deref(),
                    Some("free") | Some("x2free")
                ),
                "official": t.official_tag,
                "sticky": t.sticky,
            })
        })
        .collect();
    let has_more = p.total_estimate > page * pagesize;
    Ok(ok(serde_json::json!({
        "page": page,
        "page_size": pagesize,
        "total_estimate": p.total_estimate,
        "has_more": has_more,
        "items": items,
    })))
}

/// 种子详情（NP details 口径 + 本站扩展；download 字段为模板，passkey 由工具自行拼接）
#[get("/compat/nexusphp/torrent/{id}.json")]
async fn compat_np_torrent_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let (_uid, _rpm) = require_token(&req, &state).await?;
    let id = path.into_inner();
    let row: Option<(
        String,
        Option<String>,
        i64,
        i32,
        i32,
        i32,
        i32,
        Option<i64>,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT name, descr, size, seeders, leechers, times_completed, \
         category_id, group_id, created_at \
             FROM torrents WHERE id = $1 AND approval_status = 1",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((
        name,
        descr,
        size,
        seeders,
        leechers,
        completed,
        category,
        group_id,
        created,
    )) = row
    else {
        return Err(DomainError::NotFound(id));
    };
    Ok(ok(serde_json::json!({
        "id": id,
        "name": name,
        "descr": descr,
        "size": size,
        "seeders": seeders,
        "leechers": leechers,
        "completed": completed,
        "category": category,
        "group_id": group_id,
        "group_api": "/api/v1/torrents/{id}/group",
        "added": created.timestamp(),
        "download": "/api/v1/compat/nexusphp/download.php\
                      ?id={id}&passkey=<你的 passkey>",
    })))
}

#[derive(Deserialize)]
struct NpDownloadQuery {
    id: i64,
    passkey: String,
}

/// NP 形状下载端点：`download.php?id={id}&passkey={passkey}`。
/// 与 NexusPHP 工具生态的既有 URL 习惯一致（passkey 查询参数鉴权）。
/// 防护与网页下载同口径：过审校验 + 下载闸门（挂起/停下载拒发）+ passkey 限流。
#[get("/compat/nexusphp/download.php")]
async fn compat_np_download(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<NpDownloadQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    // passkey 限流（防枚举/暴力）：每 passkey 30 次/分钟，复用 Redis 简单计数
    let bucket = sha3_hex(q.passkey.as_bytes());
    let key = format!("rl:npdl:{}", &bucket[..16]);
    {
        use redis::AsyncCommands;
        let mut c = state.redis.clone();
        let n: i64 = c.incr(&key, 1).await.unwrap_or(0);
        if n == 1 {
            let _: () = c.expire(&key, 60).await.unwrap_or(());
        }
        if n > 30 {
            return Err(DomainError::RateLimited.into());
        }
    }
    let user_id: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE passkey = $1 AND status < 2 \
         AND download_enabled AND NOT suspended",
    )
    .bind(&q.passkey)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(user_id) = user_id else {
        return Err(DomainError::Unauthorized.into());
    };
    // 付费种子（0086）与网页下载同口径扣费：兼容端点不得绕过 charge_for_download
    charge_for_download(&state.repo.db, user_id, q.id)
        .await
        .map_err(actix_web::Error::from)?;
    let body = build_torrent_bytes(&state, user_id, q.id).await?;
    Ok(HttpResponse::Ok()
        .content_type("application/x-bittorrent")
        .body(body))
}
