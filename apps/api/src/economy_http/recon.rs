//! 产出-回收对账（0077）+ Torznab 出口（0267 合规改造）。
//! 从 economy_http.rs 按域拆出。
//!
//! Torznab 是 Prowlarr / Jackett / Sonarr / Radarr / cross-seed 的通用入口。
//! 0267 之前它「形状像但契约不到」：caps 根节点写成 `<torznab:search>`、
//! item 缺 `torznab:attr`（客户端读到 0 做种）、分类硬编码 8000、
//! `t=tvsearch/movie` 与 season/imdbid 被静默忽略 —— 结果是媒体库生态
//! 一个都接不进来。本文件按 Torznab 规范逐项补齐。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::openapi_http::{require_token, with_rl};
use crate::state::AppState;

use super::torznab_fmt::{feed_xml, tattr, xml_escape};
use crate::torrents::{newznab_name, CatMap, TorrentFilter, NEWZNAB_OTHER};

/// 经济路由（挂载进主 /api/v1 scope，单 scope 避免遮蔽）

// ============ 产出-回收对账（0077，v3 §27-22）+ Torznab 出口 ============

/// 火花产出/回收月度对账（staff）：通胀监控数据底座（v_spark_flow_monthly）
#[get("/admin/spark-flow")]
async fn spark_flow_report(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    let rows: Vec<(String, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT month, minted::bigint, burned::bigint, net::bigint, \
         entries FROM v_spark_flow_monthly LIMIT 24",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// Torznab 入口（0267 重写 + ZT2 单基址分发）。
///
/// 规范：根节点必须 `<caps>`、能力声明在 `<searching>` 下（旧实现写 `<torznab:search>`，
/// Prowlarr/Jackett 解析失败）。分类按 `categories.newznab_id` 映射，不再硬编码 8000。
/// ZT2：caps/search 原为两条独立路径，而 Prowlarr/Jackett 只配一个 API Path + `?t=`
/// 分发——配 `/torznab` 则搜索恒返 caps、配 `/torznab/search` 则 caps 解析失败，两种
/// 配法都残缺。现按 `t` 分流；`/torznab/search` 保留为兼容别名。
#[utoipa::path(
    get,
    path = "/api/v1/torznab",
    tag = "Torznab",
    description = "Torznab caps：规范的 <caps> 根节点 + <searching> 能力声明 + \
分类映射；Prowlarr/Jackett 直接可加。带 ?t= 时按 t 分流（ZT2 单基址）。",
    params(("t" = String, Query, description = "caps（缺省）/ search / tvsearch / movie")),
    responses(
        (status = 200, description = "application/xml"),
        (status = 401, description = "apikey 无效（code 2001）"),
        (status = 429, description = "限流（code 1015），带 Retry-After: 60"),
    )
)]
#[get("/torznab")]
async fn torznab_caps(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    if q.get("t").is_some_and(|v| v != "caps") {
        return torznab_search_inner(req, state, q).await;
    }
    let title: String = sqlx::query_scalar(
        "SELECT COALESCE(NULLIF((SELECT value FROM site_settings WHERE \
         name = 'SITENAME'), ''), NULLIF((SELECT value FROM site_settings \
         WHERE name = 'site_name'), ''), 'FluxTorrent')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_else(|_| "FluxTorrent".into());
    let cats = CatMap::load(&state.repo.db).await;
    let mut ids = cats.newznab_ids();
    // 兜底分类必须始终在场：映射缺失的行会归到 Other
    if !ids.contains(&NEWZNAB_OTHER) {
        ids.push(NEWZNAB_OTHER);
    }
    ids.sort_unstable();
    ids.dedup();
    let mut cats_xml = String::new();
    for id in ids {
        cats_xml.push_str(&format!(
            "    <cat id=\"{id}\" name=\"{}\" parent=\"0\" />\n",
            xml_escape(newznab_name(id)),
        ));
    }
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<caps>\n\
  <server version=\"1.0\" title=\"{title}\" url=\"/api/v1/torznab\" />\n\
  <limits max=\"100\" default=\"50\" />\n\
  <searching>\n\
    <search available=\"yes\" supportedParams=\"q\" />\n\
    <tv-search available=\"yes\" supportedParams=\"q,season,ep\" />\n\
    <movie-search available=\"yes\" supportedParams=\"q,imdbid\" />\n\
  </searching>\n\
  <categories>\n{cats_xml}  </categories>\n\
</caps>",
        title = xml_escape(&title),
    );
    Ok(HttpResponse::Ok().content_type("application/xml").body(xml))
}

/// Torznab search（0079 落地 / 0267 合规化）。
///
/// 支持的模式（与 caps 的 `<searching>` 声明一致，不再静默忽略）：
///   - `t=search`（缺省）：`q` 关键字，或 `imdbid` 走 IMDb 精确区
///   - `t=movie`   ：`imdbid` → IMDb 精确区；否则 `q`
///   - `t=tvsearch`：`q` + `season`/`ep` 收窄标题（`S01E02` token）
///   - `t=music` / `t=book`：按 `q`（本站不区分维度，等价 search）
///   - `cat`       ：逗号分隔 Newznab 分类号；命中不到任何站内分类时
///                   **返回空 feed**（而非「忽略分类照常返回」——
///                   静默忽略会让工具拿到一堆它没要的内容）
///
/// 兼容别名：把 API Path 配成 `/torznab/search` 的旧工具仍可用。
#[utoipa::path(
    get,
    path = "/api/v1/torznab/search",
    tag = "Torznab",
    description = "Torznab search：支持 t=search|tvsearch|movie（season/ep、\
imdbid/tmdbid）与 cat=Newznab 分类号；item 带完整 torznab:attr（含免费倍率）。",
    params(
        ("apikey" = String, Query, description = "Token（或用 Authorization 头）"),
        ("t" = String, Query, description = "search/tvsearch/movie"),
        ("q" = String, Query, description = "关键字"),
        ("season" = String, Query, description = "季（tvsearch）"),
        ("ep" = String, Query, description = "集（tvsearch）"),
        ("imdbid" = String, Query, description = "IMDb id（movie/search）"),
        ("cat" = String, Query, description = "Newznab 分类号，逗号分隔"),
        ("limit" = String, Query, description = "1-100，默认 50"),
        ("offset" = String, Query, description = "offset+limit ≤ 1000"),
    ),
    responses(
        (status = 200, description = "application/xml（RSS 2.0 + torznab 命名空间）"),
        (status = 401, description = "apikey 无效（code 2001）"),
        (status = 429, description = "限流（code 1015），带 Retry-After: 60"),
    )
)]
#[get("/torznab/search")]
async fn torznab_search(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    torznab_search_inner(req, state, q).await
}

/// 搜索实现（**无路由属性**）：Z T2 起由 `/torznab`（按 `t` 分发）与
/// `/torznab/search`（兼容别名）共用。actix 的 `#[get]` 宏会把同名 fn 替换成
/// 一个 service struct，故实现必须与路由项分离，路由项只做转发。
async fn torznab_search_inner(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    // 审计修复（P2）：item link/enclosure 需绝对地址，Prowlarr 等才能直接请求。
    // PUBLIC_API_URL 未配置时按请求 Host 拼。
    let api_base = std::env::var("PUBLIC_API_URL")
        .ok()
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            req.headers()
                .get("host")
                .and_then(|v| v.to_str().ok())
                .filter(|h| !h.is_empty())
                .map(|h| format!("http://{h}"))
                .unwrap_or_else(|| "http://127.0.0.1:8080".into())
        });
    let tk = require_token(&req, &state).await?;
    // enclosure/link 的 passkey 用 Token 主人的真实 passkey
    //（占位符会让 Prowlarr 拿到字面量 PASSKEY 而 401）
    let passkey: String =
        sqlx::query_scalar("SELECT passkey FROM users WHERE id = $1")
            .bind(tk.uid)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

    let get = |k: &str| -> Option<String> {
        q.get(k)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let mode = get("t").unwrap_or_else(|| "search".into());
    let keyword = get("q");
    let imdb = get("imdbid").or_else(|| get("imdb"));
    let season = get("season").and_then(|v| v.parse::<u32>().ok());
    let ep = get("ep").and_then(|v| v.parse::<u32>().ok());
    let limit: usize = get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50)
        .clamp(1, 100);
    let offset: i64 = get("offset")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
        .max(0);
    let hide_paid = get("paid").map_or(true, |v| v != "0"); // ?paid=0 要全量

    // 对外层存活口径：**全部**（含零做种）。新种在有人做种前也必须能被
    // Sonarr/Radarr 搜到，否则冷启动期工具侧永远「零结果」。
    let mut filter = TorrentFilter {
        alive: Some(0),
        ..TorrentFilter::default()
    };
    match mode.as_str() {
        "movie" => {
            // 只认 imdbid（无 tmdb 列）：Radarr 同传 q+tmdbid 时曾被顶成 0 条。
            if let Some(id) = imdb.as_ref() {
                filter.search = Some(id.clone());
                filter.search_area = Some(4);
            } else {
                filter.search = keyword.clone();
            }
        }
        "tvsearch" => {
            filter.search = keyword.clone();
            // 季集收窄：标题里的标准 `SxxExx` 标记。这是刻意的取舍 ——
            // 要么真的收窄，要么在 caps 里声明不支持；不再「收下参数但不用」。
            if let (Some(s), Some(e)) = (season, ep) {
                filter.title_like = Some(format!("S{s:02}E{e:02}"));
            } else if let Some(s) = season {
                filter.title_like = Some(format!("S{s:02}"));
            }
        }
        _ => {
            if let Some(id) = imdb.as_ref() {
                filter.search = Some(id.clone());
                filter.search_area = Some(4);
            } else {
                filter.search = keyword.clone();
            }
        }
    }

    // 只给了本站不支持的标识符（tmdbid/tvdbid）且没有任何可用条件时回
    // **空 feed**：与 cat 映射不到时同口径。把 50 条未筛列表发给以为自己在
    // 按 id 精确搜的客户端，等于让它收下一堆它没要的内容。
    if (get("tmdbid").is_some() || get("tvdbid").is_some())
        && filter.search.is_none()
        && filter.search_area.is_none()
        && filter.title_like.is_none()
    {
        return Ok(with_rl(
            HttpResponse::Ok()
                .content_type("application/xml")
                .body(feed_xml("")),
            &tk,
        ));
    }

    let cats = CatMap::load(&state.repo.db).await;
    if let Some(raw) = get("cat") {
        let want: Vec<i32> = raw
            .split(',')
            .filter_map(|s| s.trim().parse::<i32>().ok())
            .collect();
        if !want.is_empty() {
            let internal = cats.internal_ids_for_newznab(&want);
            if internal.is_empty() {
                // 明确空结果：请求的分类本站一个都没有
                return Ok(with_rl(
                    HttpResponse::Ok()
                        .content_type("application/xml")
                        .body(feed_xml("")),
                    &tk,
                ));
            }
            filter.category_id = Some(internal);
        }
    }

    // 翻页：list_torrents 内部把 limit clamp 到 50，offset≥50 时
    // 「取前 50 再 skip」恒空。改为按 offset+limit 原值直查，
    // 更深翻页按 Torznab 惯例拒绝（Prowlarr 实际只翻到 1000）。
    let fetch_n = offset + limit as i64;
    if fetch_n > 1000 {
        return Err(DomainError::Validation(
            "offset+limit 不得超过 1000".into(),
        ));
    }
    let page = crate::torrents::list_torrents_noclamp_for_sub(
        &state.repo.db,
        &filter,
        None,
        fetch_n,
        tk.uid,
        hide_paid,
    )
    .await?;
    let items: Vec<String> = page
        .items
        .into_iter()
        .skip(offset as usize)
        .take(limit)
        .map(|t| {
            let nz = cats.newznab(t.category_id);
            let kind = t.promotion.as_deref().unwrap_or("");
            let promo_tag = crate::torrents::promo::label(kind);
            let (dlf, ulf) = crate::torrents::promo::factors(kind);
            let dl = format!(
                "{}/api/v1/compat/nexusphp/download.php?id={}&passkey={}",
                api_base, t.id, passkey
            );
            let mut attrs = String::new();
            attrs.push_str(&tattr("category", &nz.to_string()));
            attrs.push_str(&tattr("size", &t.size.to_string()));
            attrs.push_str(&tattr("seeders", &t.seeders.to_string()));
            attrs.push_str(&tattr("leechers", &t.leechers.to_string()));
            attrs.push_str(&tattr(
                "peers",
                &(t.seeders + t.leechers).to_string(),
            ));
            attrs.push_str(&tattr("grabs", &t.times_completed.to_string()));
            attrs.push_str(&tattr("infohash", &t.info_hash));
            // 免费/双倍语义：媒体库据此做「优先免费种」策略。
            // 促销引擎做了这么久，不输出倍率等于白做。
            attrs.push_str(&tattr(
                "downloadvolumefactor",
                &format!("{dlf}"),
            ));
            attrs.push_str(&tattr("uploadvolumefactor", &format!("{ulf}")));
            // 促销标签：Prowlarr 按标签建索引，用户按标题肉眼识别
            if !promo_tag.is_empty() {
                attrs.push_str(&tattr("tags", &format!("free,{kind}")));
            }
            let title = if promo_tag.is_empty() {
                t.name.clone()
            } else {
                format!("[{promo_tag}] {}", t.name)
            };
            format!(
                "  <item>\n\
                 <title>{}</title>\n\
                 <guid isPermaLink=\"false\">torrent-{}</guid>\n\
                 <link>{}</link>\n\
                 <enclosure url=\"{}\" type=\"application/x-bittorrent\" length=\"{}\" />\n\
                 <pubDate>{}</pubDate>\n\
                 <size>{}</size>\n\
                 <category id=\"{}\" name=\"{}\" />\n\
{attrs}\
                 </item>\n",
                xml_escape(&title),
                t.id,
                xml_escape(&dl),
                xml_escape(&dl),
                t.size,
                t.created_at.to_rfc2822(),
                t.size,
                nz,
                xml_escape(newznab_name(nz)),
            )
        })
        .collect();
    Ok(with_rl(
        crate::openapi_http::no_store(
            HttpResponse::Ok()
                .content_type("application/xml")
                .body(feed_xml(&items.join(""))),
        ),
        &tk,
    ))
}
