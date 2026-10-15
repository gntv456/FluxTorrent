//! M08 RSS 订阅：getrss 等价 —— 按 passkey 鉴权的个性化 RSS 2.0 输出。
//! NexusPHP 口径：用户专属 token（复用 passkey）+ 可选过滤参数；刷流工具凭 URL 自动拉新种。
//! 参数对齐 getrss.php 常用子集：分类/媒介/官种/关键字/条数/标题格式/付费。
//! NP 惯用名 `cat`/`slot_number` 经 serde alias 同时受理（见 query.rs）。
//! B3（2026-09-25）起另认 `sec_{kind}` 六类型维度筛选——与前台列表、
//! 后台管理列表共用同一解析与谓词实现，三处语义不漂移。

mod authz;
mod forum;
mod query;
// utoipa::path 宏的 __path_* 辅助类型需对 compat_api 聚合可见
pub(crate) mod tokens;
mod util;

pub use util::{format_size, parse_ids, xml_escape};

use actix_web::{get, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};

use crate::state::AppState;
use query::RssQuery;

pub fn mount_rss(scope: actix_web::Scope) -> actix_web::Scope {
    tokens::mount_tokens(scope)
        .service(rss_feed)
        .service(forum::forum_rss_feed)
}

#[derive(sqlx::FromRow)]
struct RssRow {
    id: i64,
    name: String,
    small_descr: Option<String>,
    size: i64,
    created_at: DateTime<Utc>,
    official_tag: bool,
    /// 当前生效促销（rss 刷流标记用）：free/x2free/…，NULL = 无
    promotion: Option<String>,
    #[sqlx(default)]
    owner_name: Option<String>,
}

/// GET /rss/{passkey}?categories=1,2&mediums=3&official=&search=...
/// passkey 即用户身份（BEP3 同源凭证，泄露可自助 rotate —— 与 tracker 一致的暴露面）
#[utoipa::path(
    get,
    path = "/api/v1/rss/{passkey}",
    tag = "RSS",
    description = "个性化 RSS 订阅：item 含 <enclosure> 下载直链（qB RSS/\
autobrr/Flexget 直接用）；passkey 在路径里，30 次/分钟。",
    params(
        ("passkey" = String, Path,
         description = "32 位 passkey（或 fxr_ RSS token）"),
        ("categories" = String, Query, description = "分类 id 逗号多选"),
        ("mediums" = String, Query, description = "媒介 id 逗号多选"),
        ("search" = String, Query, description = "关键字"),
        ("official" = String, Query, description = "1 = 仅官种"),
        ("paid" = String, Query, description = "1 = 仅免费（free/x2free）"),
        ("showrows" = String, Query, description = "1-200，默认 50"),
        ("linktype" = String, Query, description = "page = link 指详情页且标题仅名称"),
    ),
    responses(
        (status = 200, description = "application/rss+xml"),
        (status = 400, description = "passkey 形状非法"),
        (status = 429, description = "限流（每 passkey 30 次/分钟）"),
    )
)]
#[get("/rss/{passkey}")]
pub(crate) async fn rss_feed(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<RssQuery>,
) -> HttpResponse {
    let passkey = path.into_inner();
    // 形状校验（0267/0352）：passkey 或 fxr_ token 二选一——与两个生成器
    // 口径一致，非法形状早拒省一次查询
    if !crate::compat_http::valid_passkey(&passkey)
        && !tokens::valid_token_shape(&passkey)
    {
        return HttpResponse::BadRequest().body("invalid passkey");
    }
    // 0267 安全补：RSS 此前**没有限流** —— 一枚泄露的 passkey 可无限拉订阅源
    // （打库 + 放大信息面）。30 次/分钟对分钟级轮询的订阅器绰绰有余。
    // P-68：旧写法把「限流服务不可用」也回成 429 + retry-after —— 订阅端
    // 据此退避而真实故障在 Redis，症状被伪装成「你拉太快了」。分开回。
    match crate::compat_http::limit_passkey(&state, &req, "rss", &passkey, 30)
        .await
    {
        Ok(()) => {}
        Err(crate::errors::DomainError::RateLimited) => {
            return HttpResponse::TooManyRequests()
                .insert_header(("retry-after", "60"))
                .content_type("text/plain; charset=utf-8")
                .body("too many requests");
        }
        Err(e) => {
            tracing::warn!(error = %e, "RSS 限流服务不可用");
            return HttpResponse::ServiceUnavailable()
                .content_type("text/plain; charset=utf-8")
                .body("rate limiter unavailable");
        }
    }
    // 0352：token 优先（sha3 哈希查 rss_tokens，命中顺手节流打 last_used_at），
    // miss 回落 user_by_passkey——旧 passkey URL 永远兼容
    let Some(ident) = authz::resolve_identity(&state, &passkey).await else {
        return HttpResponse::NotFound().body("unknown passkey");
    };
    let user_id = ident.user_id;

    // 多选分类/媒介：逗号分隔 → 数组（空 = 不过滤）；兼容旧版单值 category
    let categories = match parse_ids(q.categories.as_deref()) {
        Ok(v) => v.or_else(|| q.category.map(|c| vec![c])),
        Err(msg) => return HttpResponse::BadRequest().body(msg),
    };
    let mediums = match parse_ids(q.mediums.as_deref()) {
        Ok(v) => v,
        Err(msg) => return HttpResponse::BadRequest().body(msg),
    };
    // paid=1 → 仅免费促销种（当前生效的 torrent 级或 scope 级 free/x2free；修复前参数被静默忽略）
    let free_only = q.paid == Some(1);
    // 促销口径与列表/筛选谓词共用同一份实现（crate::torrents::promo）：
    // 此前 RSS 自己内联了一份命中条件 + 优先级 CASE，与主列表口径会各自漂移。
    let promo_lateral = crate::torrents::promo::lateral_latest();
    let free_clause =
        crate::torrents::promo::exists_clause(Some("'free','x2free'"));
    // B3 六类型维度筛选：与前台/后台同一实现（sec_params + section_where）。
    // 谓词是字符串片段，直接内插进 WHERE——与上面 free_clause 同一暴露面纪律。
    let sections = crate::torrent_http::parse_section_params(
        &state.repo.db,
        req.query_string(),
    )
    .await;
    let sec_sql =
        crate::torrents::section_where(&state.repo.db, &sections).await;
    // 深测 2026-10-03：付费种子无差别推给订阅器——未购用户下载时才被
    // charge_for_download 拦下（402）。默认排除「付费且未购」；paid=0 全量。
    let exclude_paid = q.paid != Some(0);
    // 0375：RSS 存活口径开关。默认 no = 保持「全部」（刷流器既有订阅
    // 按「全部」在跑，直接翻转会让它们突然少掉源种）。改 yes 时接的是
    // **同一份** seeders > 0 谓词，不在 RSS 里另写一份（另写会与主
    // 列表口径漂移，正是审计里反复出现的病根）。
    let alive_only: bool = sqlx::query_scalar(
        "SELECT COALESCE((value = $1), false) FROM site_settings WHERE name = $2"
    )
    .bind("yes")
    .bind("rss_alive_only")
    .fetch_optional(&state.repo.db)
    .await
    .unwrap_or(None)
    .unwrap_or(false);
    let sql = format!(
        "SELECT t.id, t.name, t.small_descr, t.size, t.created_at, \
                t.official_tag, pr.promotion, \
            CASE WHEN t.anonymous THEN NULL ELSE u.username END AS owner_name \
         FROM torrents t LEFT JOIN users u ON u.id = t.owner_id \
         {promo_lateral} \
         WHERE t.approval_status = 1 \
           AND ($1::int[] IS NULL OR t.category_id = ANY($1)) \
           AND ($2::int[] IS NULL OR \
                t.medium_id = ANY($2) \
                OR EXISTS (SELECT 1 FROM torrent_sections ts \
                           JOIN section_dict sd ON sd.id = ts.dict_id \
                           WHERE ts.torrent_id = t.id AND ts.kind = 'media' \
                           AND sd.name IN (SELECT name FROM media \
                                           WHERE id = ANY($2)))) \
           AND ($3::bool IS NULL OR t.official_tag = $3) \
           AND ($4::text IS NULL OR t.name ILIKE '%' || $4 || '%' \
                ESCAPE chr(92)) \
           AND (NOT $6::bool OR {free_clause}){sec_sql} \
           AND (NOT $7::bool OR t.price <= 0 OR t.owner_id = $8 \
                OR EXISTS (SELECT 1 FROM torrent_purchases tp \
                           WHERE tp.user_id = $8 AND tp.torrent_id = t.id)) \
           AND (NOT $9::bool OR t.seeders > 0) \
         ORDER BY t.id DESC LIMIT $5"
    );
    let rows: Vec<RssRow> = sqlx::query_as(&sql)
        .bind(categories.as_deref())
        .bind(mediums.as_deref())
        .bind(q.official)
        // LIKE 通配转义（2026-10-10 批十）：与主列表 esc_like 同口径——裸 %/_ 当
        // 通配会让 search=% 退化成「不过滤」，RSS 与列表两个出口语义漂移
        .bind(
            q.search
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(crate::torrents::esc_like),
        )
        .bind(q.showrows.unwrap_or(50).clamp(1, 200))
        .bind(free_only)
        .bind(exclude_paid)
        .bind(user_id)
        .bind(alive_only)
        .fetch_all(&state.repo.db)
        .await
        .unwrap_or_default();

    // 审计修复（P2）：PUBLIC_SITE_URL 未配置时退请求 Host 拼绝对地址，
    // RSS 阅读器才能跳转（此前 channel link 为空、item link 为相对路径）
    let base = std::env::var("PUBLIC_SITE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| {
            let host = req
                .headers()
                .get("host")
                .and_then(|v| v.to_str().ok())
                .map(str::trim)
                .filter(|h| crate::compat_http::aliases::sane_host(h))
                .map(|h| format!("http://{h}"))
                .unwrap_or_else(|| "http://localhost:3000".into());
            host
        });
    // 促销标记（刷流时效性关键，NP 官方插件口径）：Free/2xFree 必须在 RSS
    // 标题第一时间可见——刷流器按标题关键词过滤，标记缺失 = 免费信息传不到。
    // 0267：六档标签收进 promo::label 单一来源（此前 RSS/Torznab 各写一份）。
    let promo_tag = |p: &Option<String>| -> String {
        match p.as_deref().map(crate::torrents::promo::label) {
            Some(l) if !l.is_empty() => format!("[{l}] "),
            _ => String::new(),
        }
    };
    let verbose = q.linktype.as_deref() != Some("page");
    // 下载直链基址：优先 PUBLIC_API_URL（API 独立域名/端口场景），
    // 否则回落站点基址（生产同域反代 /api → api 服务）。
    let api_base = std::env::var("PUBLIC_API_URL")
        .ok()
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| base.clone());
    let mut items = String::new();
    for r in &rows {
        let title = if verbose {
            format!(
                "{}{}{} {} {} · {}",
                promo_tag(&r.promotion),
                if r.official_tag { "[官种] " } else { "" },
                r.name,
                r.small_descr.clone().unwrap_or_default(),
                format_size(r.size),
                r.owner_name.clone().unwrap_or_default(),
            )
        } else {
            format!("{}{}", promo_tag(&r.promotion), r.name)
        };
        // 0267 修复：此前 item 既无 <enclosure>、<link> 又指向网页，刷流器
        // （qBittorrent RSS / autobrr / Flexget）拿到 HTML → 刷流链路不可用。
        // 现在：link = NP 口径下载直链；enclosure = 同一地址。linktype=page
        // 时保留旧行为（注意：linktype 管**标题格式**，与下载链接无关）。
        // 0352：直链凭证跟随鉴权路径——token 消费时放 token（订阅 URL 与
        // 下载直链同凭证，撤销一个 token 即整链失效），passkey 时原样。
        let dl = format!(
            "{}/api/v1/compat/nexusphp/download.php?id={}&passkey={}",
            api_base, r.id, ident.credential
        );
        let link = if verbose {
            dl.clone()
        } else {
            format!("{}/torrent/{}", base, r.id)
        };
        items.push_str(&format!(
            "<item><title>{}</title><link>{}</link>\
             <guid isPermaLink=\"true\">{}/torrent/{}</guid>\
             <enclosure url=\"{}\" type=\"application/x-bittorrent\" \
             length=\"{}\"/>\
             <pubDate>{}</pubDate><description>{} · {}</description></item>",
            xml_escape(&title),
            xml_escape(&link),
            base,
            r.id,
            xml_escape(&dl),
            r.size,
            r.created_at.format("%a, %d %b %Y %H:%M:%S GMT"),
            xml_escape(&r.small_descr.clone().unwrap_or_default()),
            format_size(r.size),
        ));
    }
    // 频道名/描述跟随站点设定，不再硬编码品牌（通用 PT 建站系统，站点身份由站长在「站点设定」配置）。
    // SITENAME（NP 口径=全站标题/RSS 频道名）→ site_name → FluxTorrent。
    let channel_title: String = sqlx::query_scalar(
        "SELECT COALESCE(NULLIF((SELECT value FROM site_settings \
                                 WHERE name = 'SITENAME'), ''), \
                         NULLIF((SELECT value FROM site_settings \
                                 WHERE name = 'site_name'), ''), \
                         'FluxTorrent')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_else(|_| "FluxTorrent".into());
    let channel_desc: String = sqlx::query_scalar(
        "SELECT COALESCE(NULLIF((SELECT value FROM site_settings \
                                 WHERE name = 'site_desc'), ''), \
                         NULLIF((SELECT value FROM site_settings \
                                 WHERE name = 'metadescription'), ''), \
                         '私有种子社区')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_else(|_| "私有种子社区".into());
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <rss version=\"2.0\"><channel>\
         <title>{} 最新种子</title>\
         <link>{}</link><description>{} RSS</description>\
         <ttl>15</ttl>{}</channel></rss>",
        xml_escape(&channel_title),
        base,
        xml_escape(&channel_desc),
        items
    );
    HttpResponse::Ok()
        .content_type("application/rss+xml; charset=utf-8")
        // 0267：enclosure/link 里带本人 passkey，绝不能被中间层缓存
        .insert_header(("cache-control", "private, no-store, max-age=0"))
        .body(xml)
}
