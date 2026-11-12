//! 生态兼容层 ②：NexusPHP 字段口径的只读 JSON + download.php 形状下载。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::np_query::{resolve_alive, NpListQuery};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::openapi_http::{no_store, require_token, with_rl};
use crate::publish_http::build_torrent_bytes;
use crate::state::AppState;
use crate::torrents::{
    charge_for_download, ensure_downloadable, list_torrents_noclamp, CatMap,
    MediaMap, TorrentFilter,
};

// ============ NexusPHP 兼容端点（Token 鉴权，只读） ============

/// 当前 Token 所属用户（NP userdetails 口径，不含任何 Tracker 凭据）
#[utoipa::path(
    get,
    path = "/api/v1/compat/nexusphp/user.json",
    tag = "兼容层(NP)",
    description = "NP userdetails 口径账号信息。**不再返回 passkey**：\
Tracker 身份只在网页会话可见，开放 API Token 不能换出长期汇报密钥。\
分享率恒非负，downloaded=0 时 ratio_infinite=true。",
    responses(
        (status = 200, description = "NpUser"),
        (status = 401, description = "Token 无效/已撤销/未传（code 2001）"),
        (status = 429, description = "限流（code 1015），带 Retry-After: 60"),
    )
)]
#[get("/compat/nexusphp/user.json")]
async fn compat_np_user(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    let body = super::nexusphp_user::np_user_json(&state, tk.uid).await?;
    Ok(no_store(with_rl(ok(body), &tk)))
}

/// 种子列表（NP torrents.php 字段口径，游标分页内部转 page 语义）
#[utoipa::path(
    get,
    path = "/api/v1/compat/nexusphp/torrents.json",
    tag = "兼容层(NP)",
    description = "NP 口径种子列表（高级筛选）：category 仍是站内 id；\
新增 category_name/category_np/category_newznab、info_hash/pieces_hash。\
默认含零做种新种。",
    params(
        ("page" = String, Query, description = "1-400"),
        ("pagesize" = String, Query, description = "≤50，默认 30"),
        ("keyword" = String, Query, description = "关键字（标题）"),
        ("category" = String, Query, description = "站内分类 id"),
        ("search_area" = String, Query, description = "0=标题 1=简介 3=发布者 4=IMDb"),
        ("imdb" = String, Query, description = "IMDb id（等效 search_area=4）"),
        ("alive" = String, Query, description = "0=全部(默认) 1=活种 2=断种"),
        ("promo" = String, Query, description = "free/x2/half/any/none，逗号多选"),
        ("size_min" = String, Query, description = "字节数或 5GB/500MB/2TB"),
        ("size_max" = String, Query, description = "同上"),
        ("min_seeders" = String, Query, description = "做种数下界（含）"),
        ("max_seeders" = String, Query, description = "做种数上界（含）"),
        ("date_from" = String, Query, description = "YYYY-MM-DD（含当天）"),
        ("date_to" = String, Query, description = "YYYY-MM-DD（含当天）"),
        ("owner" = String, Query, description = "发布者用户名（模糊）"),
        ("official" = String, Query, description = "1 = 仅官种"),
        ("sort" = String, Query,
         description = "created/seeders/size/completed（可加 _asc）"),
        ("tags" = String, Query, description = "tag_dict.id 逗号串"),
        ("tag_mode" = String, Query, description = "any（默认）/ all"),
    ),
    responses(
        (status = 200, description = "NpTorrentList：items + has_more 分页语义"),
        (status = 401, description = "Token 无效/已撤销/未传（code 2001）"),
        (status = 429, description = "限流（code 1015），带 Retry-After: 60"),
    )
)]
#[get("/compat/nexusphp/torrents.json")]
async fn compat_np_torrents(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<NpListQuery>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    // ZT81：page 必须有上界——page*pagesize 直接进 SQL LIMIT（实测 page=1000000
    // 触发 LIMIT 50,000,000，20,000 种子时 1.65s；十万级即可被单请求打满 DB）。
    let page = q.page.unwrap_or(1).clamp(1, 400);
    let pagesize = q.pagesize.unwrap_or(30).clamp(1, 50);
    // IMDb 参数优先（NP 工具惯用 ?imdb=tt123）；否则用显式 search_area
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
    // 0268：高级筛选走与站内搜索**同一套**归一器（体积/优惠/日期/标签），
    // 复制一份必然漂移。布尔参数取 String——老工具发 `official=1`，bool 会炸。
    let on = |v: &Option<String>| -> Option<bool> {
        match v.as_deref().map(str::trim) {
            Some("1") | Some("true") | Some("yes") => Some(true),
            Some("0") | Some("false") | Some("no") => Some(false),
            _ => None,
        }
    };
    let tag_parts: Vec<String> = q
        .tags
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(str::to_string)
        .collect();
    let (tag_ids, tag_all) =
        crate::torrent_http::norm_tags(None, &tag_parts, q.tag_mode.as_deref());
    let filter = TorrentFilter {
        category_id: q.category.map(|v| vec![v]),
        search,
        search_area,
        search_mode: q.search_mode,
        // 对外缺省「全部」（含零做种）：新种必须立刻可被第三方搜到
        alive: Some(resolve_alive(q.alive, q.include_dead.as_deref())),
        promo: crate::torrent_http::promo_or_err(q.promo.clone())?,
        size_min: crate::torrent_http::size_or_err(q.size_min.clone())?,
        size_max: crate::torrent_http::size_or_err(q.size_max.clone())?,
        min_seeders: q.min_seeders.filter(|v| *v >= 0),
        max_seeders: q.max_seeders.filter(|v| *v >= 0),
        date_from: crate::torrent_http::date_or_err(q.date_from.clone())?,
        date_to: crate::torrent_http::date_or_err(q.date_to.clone())?,
        owner: crate::torrent_http::norm_text(q.owner.clone()),
        official: on(&q.official),
        sort: q
            .sort
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        tag_ids,
        tag_all,
        ..TorrentFilter::default()
    };
    // 分类/媒介小字典：一次载入，供本页每行补可读名与标准号
    let cats = CatMap::load(&state.repo.db).await;
    let media = MediaMap::load(&state.repo.db).await;
    // 审计修复（P1）：list_torrents 内部钳 limit≤50，page≥2 时 skip 后恒空，
    // 深翻页走 noclamp 版（调用方已 clamp pagesize≤50）。
    let limit = page * pagesize;
    let p = list_torrents_noclamp(&state.repo.db, &filter, None, limit).await?;
    let skip = ((page - 1) * pagesize) as usize;
    let items: Vec<_> = p
        .items
        .into_iter()
        .skip(skip)
        .take(pagesize as usize)
        .map(|t| {
            let cat = t.category_id;
            serde_json::json!({
                "id": t.id,
                "name": t.name,
                "small_descr": t.small_descr,
                "seeders": t.seeders,
                "leechers": t.leechers,
                "completed": t.times_completed,
                "size": t.size,
                "added": t.created_at.timestamp(),
                // 兼容既有消费方：category 仍是内部 id（只增不改）
                "category": cat,
                // 0267：可读名 + 两种标准号 + 辅种指纹，工具无需自备映射表
                "category_name": cats.name(cat),
                "category_np": cats.legacy(cat),
                "category_newznab": cats.newznab(cat),
                "medium": t.medium_id,
                "medium_name": t.medium_id.and_then(|m| media.name(m)),
                "promotion": t.promotion,
                // 促销可读标签（Free / 2xFree / 50% …），与 RSS/Torznab 同源
                "promotion_name": t
                    .promotion
                    .as_deref()
                    .map(crate::torrents::promo::label)
                    .filter(|s| !s.is_empty()),
                "info_hash": t.info_hash,
                "pieces_hash": t.pieces_hash,
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
    // 行内含 info_hash/pieces_hash 与 download 模板：禁缓存
    Ok(no_store(with_rl(
        ok(serde_json::json!({
            "page": page,
            "page_size": pagesize,
            "total_estimate": p.total_estimate,
            "has_more": has_more,
            "items": items,
        })),
        &tk,
    )))
}

/// 种子详情（NP details 口径 + 本站扩展；download 模板使用 RSS token）
#[utoipa::path(
    get,
    path = "/api/v1/compat/nexusphp/torrent/{id}.json",
    tag = "兼容层(NP)",
    description = "NP 口径种子详情：含双指纹（info_hash/pieces_hash）与 download 模板。",
    responses(
        (status = 200, description = "NpTorrentDetail"),
        (status = 401, description = "Token 无效/已撤销/未传（code 2001）"),
        (status = 404, description = "资源不存在（code 1004）"),
        (status = 429, description = "限流（code 1015），带 Retry-After: 60"),
    )
)]
#[get("/compat/nexusphp/torrent/{id}.json")]
async fn compat_np_torrent_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    let id = path.into_inner();
    let row: Option<(
        String,
        Option<String>,
        String,
        Option<String>,
        i64,
        i32,
        i32,
        i32,
        i32,
        Option<i32>,
        Option<i64>,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT info_hash, pieces_hash, name, descr, size, seeders, \
         leechers, times_completed, category_id, medium_id, group_id, \
         created_at FROM torrents WHERE id = $1 AND approval_status = 1",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    #[rustfmt::skip]
    let Some((info_hash, pieces_hash, name, descr, size, seeders, leechers,
        completed, category, medium_id, group_id, created)) = row
    else {
        return Err(DomainError::NotFound(id));
    };
    let cats = CatMap::load(&state.repo.db).await;
    let media = MediaMap::load(&state.repo.db).await;
    Ok(with_rl(
        ok(serde_json::json!({
            "id": id,
            "name": name,
            "descr": descr,
            "size": size,
            "seeders": seeders,
            "leechers": leechers,
            "completed": completed,
            "category": category,
            "category_name": cats.name(category),
            "category_np": cats.legacy(category),
            "category_newznab": cats.newznab(category),
            "medium": medium_id,
            "medium_name": medium_id.and_then(|m| media.name(m)),
            // 0267：辅种/查重工具需要的双指纹
            "info_hash": info_hash,
            "pieces_hash": pieces_hash,
            "group_id": group_id,
            "group_api": "/api/v1/torrents/{id}/group",
            "added": created.timestamp(),
            "download": "/api/v1/compat/nexusphp/download.php\
                          ?id={id}&token=<你的 fxr_ token>",
        })),
        &tk,
    ))
}

#[derive(Deserialize)]
struct NpDownloadQuery {
    id: i64,
    /// 旧工具仍把这个字段叫 passkey；值必须是专用 token。
    passkey: Option<String>,
    token: Option<String>,
    /// 开放 API Token（Torznab/Prowlarr 抓取用）。
    apikey: Option<String>,
}

/// NP 形状下载端点：`download.php?id={id}&token={fxr_...}`。
/// 旧参数名 `passkey=` 仍可承载 token；防护与网页下载同口径。
#[utoipa::path(
    get,
    path = "/api/v1/compat/nexusphp/download.php",
    tag = "兼容层(NP)",
    description = "NP 形状下载（专用 token 鉴权，免 cookie）：返回 .torrent，\
内嵌本人 announce。`token` 或旧字段名 `passkey` 传 fxr_ token；\
原始 32 位 passkey 一律 401。每 token 30 次/分钟。",
    params(
        ("id" = String, Query, description = "种子 id（必填）"),
        ("token" = String, Query, description = "fxr_ token"),
        ("apikey" = String, Query, description = "开放 API Token fxo_"),
        ("passkey" = String, Query,
         description = "旧字段名，仅接受 fxr_ token"),
    ),
    responses(
        (status = 200, description = "application/x-bittorrent"),
        (status = 401, description = "token 无效（code 2001）"),
        (status = 404, description = "资源不存在（code 1004）"),
        (status = 429, description = "限流（code 1015），带 Retry-After: 60"),
    )
)]
#[get("/compat/nexusphp/download.php")]
async fn compat_np_download(
    state: web::Data<std::sync::Arc<AppState>>,
    req: HttpRequest,
    q: crate::http::Q<NpDownloadQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    // API Token 走统一 Token 闸；fxr_ token 走订阅闸；原始 passkey 不认。
    let user_id = if q.apikey.is_some() {
        // apikey 分支同过 IP 维限流 + scope 闸：不比 token 分支宽松，
        // 否则是无 IP 上限的探测面，且窄 scope 的 fxo_ 会借道获得下载面。
        let ak = q.apikey.as_deref().unwrap_or("");
        super::limit_passkey(&state, &req, "npdl", ak, 30)
            .await
            .map_err(actix_web::Error::from)?;
        let tk = crate::openapi_http::require_token(&req, &state).await?;
        crate::openapi_http::require_scope(
            &tk,
            crate::openapi_http::SCOPE_READ,
        )
        .map_err(actix_web::Error::from)?;
        tk.uid
    } else {
        let cred = q
            .token
            .as_deref()
            .filter(|v| !v.is_empty())
            .or(q.passkey.as_deref().filter(|v| !v.is_empty()))
            .filter(|v| crate::rss_http::tokens::valid_token_shape(v))
            .ok_or_else(|| actix_web::Error::from(DomainError::Unauthorized))?;
        // 凭证维限流：每 token 30 次/分钟 + IP 维 120/min。fail-close。
        super::limit_passkey(&state, &req, "npdl", cred, 30)
            .await
            .map_err(actix_web::Error::from)?;
        // token 路径必须过撤销与账号状态两道闸，换凭证不放宽。
        let uid = crate::rss_http::authz::resolve_user_id(&state, cred)
            .await
            .map_err(actix_web::Error::from)?;
        uid.ok_or_else(|| actix_web::Error::from(DomainError::Unauthorized))?
    };
    // 这一路只判「凭证是不是本人」；「停下载/挂起」由 ensure_downloadable
    // 给可行动原话（P-164），且**必须在扣费之前**（P-162）。
    ensure_downloadable(&state.repo.db, user_id)
        .await
        .map_err(actix_web::Error::from)?;
    // 付费种子（0086）与网页下载同口径扣费：兼容端点不得绕过 charge_for_download
    charge_for_download(&state.repo.db, user_id, q.id)
        .await
        .map_err(actix_web::Error::from)?;
    let body = build_torrent_bytes(&state, user_id, q.id).await?;
    // .torrent 内嵌本人 announce（含 passkey）：禁止中间层缓存
    Ok(crate::openapi_http::no_store(
        HttpResponse::Ok()
            .content_type("application/x-bittorrent")
            .body(body),
    ))
}
