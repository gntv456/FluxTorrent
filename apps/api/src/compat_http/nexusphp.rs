//! 生态兼容层 ②：NexusPHP 字段口径的只读 JSON + download.php 形状下载。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::common::sha3_hex;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::openapi_http::{no_store, require_token, with_rl};
use crate::publish_http::build_torrent_bytes;
use crate::state::AppState;
use crate::torrents::{
    charge_for_download, list_torrents_noclamp, CatMap, MediaMap, TorrentFilter,
};

// ============ NexusPHP 兼容端点（Token 鉴权，只读） ============

/// 当前 Token 所属用户（NP userdetails 口径：uploaded/downloaded/seedbonus/passkey）
#[get("/compat/nexusphp/user.json")]
async fn compat_np_user(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    let body = np_user_json(&state, tk.uid).await?;
    // 含本人 passkey：禁止中间层缓存
    Ok(no_store(with_rl(ok(body), &tk)))
}

/// NP userdetails 口径的用户 JSON（`user.json` 与别名 `userdetails.php` 共用，
/// 保证两条路径的字段口径不可能漂移）。
pub(super) async fn np_user_json(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
) -> DomainResult<serde_json::Value> {
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
    // 分享率：downloaded=0 时旧实现回 -1.0，工具会当负数直接显示。
    // 改为恒非负 + 语义标记（0267）：真值 >0 时给数值；只上传未下载给
    // ratio_infinite=true 与 display "∞"；两者皆 0 给 "---"。
    let (ratio, ratio_infinite, ratio_display) = if downloaded > 0 {
        let r =
            (uploaded as f64 / downloaded as f64 * 10000.0).round() / 10000.0;
        (r, false, format!("{r:.4}"))
    } else if uploaded > 0 {
        (0.0, true, "∞".to_string())
    } else {
        (0.0, false, "---".to_string())
    };
    // 等级：NP 口径给可读等级名（class 数字对工具没有展示价值）
    let class_name: String =
        sqlx::query_scalar("SELECT name FROM user_classes WHERE id = $1")
            .bind(class_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .unwrap_or_else(|| format!("LV{class_id}"));
    Ok(serde_json::json!({
        "id": id,
        "username": username,
        "uploaded": uploaded,
        "downloaded": downloaded,
        "seedbonus": seedbonus,
        "class": class_id,
        "class_name": class_name,
        "ratio": ratio,
        "ratio_infinite": ratio_infinite,
        "ratio_display": ratio_display,
        "passkey": passkey, // 本人 Token 才能读取；工具用它拼接 download.php 与 tracker 汇报
    }))
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
    /// 存活筛选（0102 内部口径）：0=全部 1=仅活种 2=仅断种；
    /// **对外缺省 = 0（全部）** —— 新种在有人做种前也必须对第三方可见，
    /// 否则冷启动期工具搜索恒空（被误判成「对接坏了」）。
    #[serde(default)]
    alive: Option<i16>,
    /// 旧习惯兼容：1/true=全部（含断种） 0/false=仅活种。给了 alive 时以 alive 为准。
    /// 取 String 而非 bool —— 老工具常发 `include_dead=1`，serde 的 bool
    /// 遇 "1" 会整条 Query 反序列化失败（此项目历史上踩过同名坑）。
    #[serde(default)]
    include_dead: Option<String>,
    // ===== 0268：对外层透出站内已有的高级筛选（此前只有 9 个参数，
    // 「只看 free、>5GB、做种<3」这类刷流规则做不了；口径与网页搜索一致） =====
    /// 优惠筛选：free / x2 / half / any / none，支持逗号多选（与站内搜索同口径）
    #[serde(default)]
    promo: Option<String>,
    /// 体积下/上界：字节数或 `5GB`/`500MB`/`2TB`（与站内搜索同一解析器）
    #[serde(default)]
    size_min: Option<String>,
    #[serde(default)]
    size_max: Option<String>,
    /// 做种数下/上界（含）
    #[serde(default)]
    min_seeders: Option<i32>,
    #[serde(default)]
    max_seeders: Option<i32>,
    /// 发布时间范围（`YYYY-MM-DD`，含当天；非法值丢弃不筛）
    #[serde(default)]
    date_from: Option<String>,
    #[serde(default)]
    date_to: Option<String>,
    /// 发布者用户名（模糊）
    #[serde(default)]
    owner: Option<String>,
    /// 官种：1/true = 仅官种
    #[serde(default)]
    official: Option<String>,
    /// 排序：created(默认)/seeders/size/completed，可加 `_asc` 反转
    #[serde(default)]
    sort: Option<String>,
    /// 标签：tag_dict.id 逗号串；`tag_mode=any|all`（缺省 any）
    #[serde(default)]
    tags: Option<String>,
    #[serde(default)]
    tag_mode: Option<String>,
}

/// 解析对外存活口径：alive 优先，其次 include_dead，最后缺省「全部」。
fn resolve_alive(alive: Option<i16>, include_dead: Option<&str>) -> i16 {
    if let Some(v) = alive {
        if (0..=2).contains(&v) {
            return v;
        }
    }
    match include_dead.map(str::trim) {
        Some("0") | Some("false") | Some("no") => 1,
        _ => 0,
    }
}

/// 种子列表（NP torrents.php 字段口径，游标分页内部转 page 语义）
#[get("/compat/nexusphp/torrents.json")]
async fn compat_np_torrents(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<NpListQuery>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    // ZT81：page 必须有上界——page*pagesize 直接进 SQL LIMIT（实测 page=1000000
    // 触发 LIMIT 50,000,000，20,000 种子时 1.65s；十万级即可被单请求打满 DB）。
    let page = q.page.unwrap_or(1).clamp(1, 400);
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
    // 0268：高级筛选走与站内搜索**同一套**归一器（体积/优惠/日期/标签），
    // 复制一份必然漂移。布尔类参数取 String —— 老工具发 `official=1`，bool 会炸。
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
        promo: crate::torrent_http::norm_promo(q.promo.clone()),
        size_min: crate::torrent_http::parse_size(q.size_min.clone()),
        size_max: crate::torrent_http::parse_size(q.size_max.clone()),
        min_seeders: q.min_seeders.filter(|v| *v >= 0),
        max_seeders: q.max_seeders.filter(|v| *v >= 0),
        date_from: crate::torrent_http::norm_date(q.date_from.clone()),
        date_to: crate::torrent_http::norm_date(q.date_to.clone()),
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
                // 兼容既有消费方：category 仍是内部 id（不动语义，只增不改）
                "category": cat,
                // 0267 新增：可读名 + 两种标准号，工具无需自备映射表
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
                // 跨站辅种指纹（0267）：IYUU/cross-seed 类工具按 hash 精确匹配
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

/// 种子详情（NP details 口径 + 本站扩展；download 字段为模板，passkey 由工具自行拼接）
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
         created_at \
             FROM torrents WHERE id = $1 AND approval_status = 1",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((
        info_hash,
        pieces_hash,
        name,
        descr,
        size,
        seeders,
        leechers,
        completed,
        category,
        medium_id,
        group_id,
        created,
    )) = row
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
                          ?id={id}&passkey=<你的 passkey>",
        })),
        &tk,
    ))
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
    req: HttpRequest,
    q: web::Query<NpDownloadQuery>,
) -> Result<HttpResponse, actix_web::Error> {
    // passkey 限流（防枚举/暴力）：每 passkey 30 次/分钟 + IP 维 120/min
    // （换随机 passkey 打散桶的资源面，与 limit_passkey 同口径）
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
        let ip_key = format!("rl:npdl-ip:{}", crate::http::client_ip(&req));
        let m: i64 = c.incr(&ip_key, 1).await.unwrap_or(0);
        if m == 1 {
            let _: () = c.expire(&ip_key, 60).await.unwrap_or(());
        }
        if m > 120 {
            return Err(DomainError::RateLimited.into());
        }
    }
    let user_id: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM user_by_passkey WHERE passkey = $1 AND status < 2 \
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
    // .torrent 内嵌本人 announce（含 passkey）：禁止中间层缓存
    Ok(crate::openapi_http::no_store(
        HttpResponse::Ok()
            .content_type("application/x-bittorrent")
            .body(body),
    ))
}
