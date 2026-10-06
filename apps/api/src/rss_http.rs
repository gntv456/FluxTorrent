//! M08 RSS 订阅：getrss 等价 —— 按 passkey 鉴权的个性化 RSS 2.0 输出。
//!
//! NexusPHP 口径：用户专属 token（我们复用 passkey）+ 可选过滤参数；
//! 刷流工具（RSS 阅读器/下载器）凭 URL 自动拉新种。
//! 参数对齐参考站 getrss.php 的常用子集：分类多选/媒介多选/官种/关键字/条数/标题格式/付费。
//! B3（2026-09-25）起另认 `sec_{kind}` 六类型维度筛选——与前台列表、
//! 后台管理列表共用同一解析与谓词实现，三处语义不漂移。

mod forum;

use actix_web::{get, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};

use crate::state::AppState;

pub fn mount_rss(scope: actix_web::Scope) -> actix_web::Scope {
    scope.service(rss_feed).service(forum::forum_rss_feed)
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

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// GET /rss/{passkey}?categories=1,2&mediums=3&official=&search=&showrows=&linktype=&paid=
/// passkey 即用户身份（BEP3 同源凭证，泄露可自助 rotate —— 与 tracker 一致的暴露面）
#[get("/rss/{passkey}")]
async fn rss_feed(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<RssQuery>,
) -> HttpResponse {
    let passkey = path.into_inner();
    // 形状校验收紧（0267）：此前只查长度 32，现在与 passkey 生成口径一致
    // （32 位小写字母数字）——非法字符不可能命中库，早拒省一次查询
    if !crate::compat_http::valid_passkey(&passkey) {
        return HttpResponse::BadRequest().body("invalid passkey");
    }
    // 0267 安全补：RSS 此前**没有限流** —— 一枚泄露的 passkey 可无限拉订阅源
    // （打库 + 放大信息面）。30 次/分钟对分钟级轮询的订阅器绰绰有余。
    if crate::compat_http::limit_passkey(&state, "rss", &passkey, 30)
        .await
        .is_err()
    {
        return HttpResponse::TooManyRequests()
            .insert_header(("retry-after", "60"))
            .content_type("text/plain; charset=utf-8")
            .body("too many requests");
    }
    let user: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE passkey = $1 AND status < 2",
    )
    .bind(&passkey)
    .fetch_optional(&state.repo.db)
    .await
    .unwrap_or(None);
    let Some(user_id) = user else {
        return HttpResponse::NotFound().body("unknown passkey");
    };

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
    // 深测 2026-10-03：付费种子无差别推给订阅器 —— 未购用户下载时才被
    // charge_for_download 拦下（402），刷流器每轮重试同一批失败项形成噪音。
    // 默认排除「付费且该用户未购」的种子；paid=0 显式要求全量时才放行。
    let exclude_paid = q.paid != Some(0);
    let sql = format!(
        "SELECT t.id, t.name, t.small_descr, t.size, t.created_at, t.official_tag, \
                pr.promotion, \
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
           AND ($4::text IS NULL OR t.name ILIKE '%' || $4 || '%') \
           AND (NOT $6::bool OR {free_clause}){sec_sql} \
           AND (NOT $7::bool OR t.price <= 0 OR t.owner_id = $8 \
                OR EXISTS (SELECT 1 FROM torrent_purchases tp \
                           WHERE tp.user_id = $8 AND tp.torrent_id = t.id)) \
         ORDER BY t.id DESC LIMIT $5"
    );
    let rows: Vec<RssRow> = sqlx::query_as(&sql)
        .bind(categories.as_deref())
        .bind(mediums.as_deref())
        .bind(q.official)
        .bind(q.search.as_deref().filter(|s| !s.is_empty()))
        .bind(q.showrows.unwrap_or(50).clamp(1, 200))
        .bind(free_only)
        .bind(exclude_paid)
        .bind(user_id)
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
                .filter(|h| !h.is_empty())
                .map(|h| format!("http://{h}"))
                .unwrap_or_else(|| "http://localhost:3000".into());
            host
        });
    // 促销标记（刷流时效性关键，NP 官方插件口径）：用户自购/站方挂的 Free 与 2xFree
    // 必须在 RSS 标题第一时间可见——刷流器按标题关键词过滤，标记缺失 = 免费信息传不到
    // （NP 二改站的常见缺陷）。标记格式与列表角标一致：Free / 2xFree / 50% / 2x / 2x50%。
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
        // 0267 修复：此前 item 既无 <enclosure>、<link> 又指向网页，
        // 刷流器（qBittorrent RSS 自动下载 / autobrr / Flexget / cross-seed）
        // 拿到的是 HTML 页面 → 整条刷流链路不可用。现在：
        //   link      = NP 口径的下载直链（刷流器直接取 link 即可）
        //   enclosure = 同一地址（标准 RSS 消费方走 enclosure）
        // linktype=page 时保留「link 指向详情页、标题仅名称」的旧行为
        // （注意：linktype 管的是**标题格式**，与下载链接无关，历史上被误读）。
        let dl = format!(
            "{}/api/v1/compat/nexusphp/download.php?id={}&passkey={}",
            api_base, r.id, passkey
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
    // SITENAME（NexusPHP 口径=全站标题/RSS 频道名）→ site_name → FluxTorrent；site_desc → metadescription → 通用。
    let channel_title: String = sqlx::query_scalar(
        "SELECT COALESCE(NULLIF((SELECT value FROM site_settings WHERE name = 'SITENAME'), ''), \
                         NULLIF((SELECT value FROM site_settings WHERE name = 'site_name'), ''), \
                         'FluxTorrent')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_else(|_| "FluxTorrent".into());
    let channel_desc: String = sqlx::query_scalar(
        "SELECT COALESCE(NULLIF((SELECT value FROM site_settings WHERE name = 'site_desc'), ''), \
                         NULLIF((SELECT value FROM site_settings WHERE name = 'metadescription'), ''), \
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

#[derive(serde::Deserialize)]
struct RssQuery {
    /// 旧版单值兼容
    category: Option<i32>,
    categories: Option<String>,
    mediums: Option<String>,
    official: Option<bool>,
    search: Option<String>,
    showrows: Option<i64>,
    /// dl = 标题带元信息（默认）；page = 仅标题
    linktype: Option<String>,
    /// 0=全部（含未购付费种） 1=仅免费促销（free/x2free；与好学 paid 口径对齐）；
    /// 缺省 = 排除「付费且未购」的种子（深测 2026-10-03：付费种无差别推送会让
    /// 刷流器反复拿到 402，默认挡掉；需要全量监控付费种的用户显式传 paid=0）
    paid: Option<i32>,
}

/// 逗号分隔 id 串 → 数组。空/未给 = 不过滤（Ok(None)）；
/// 给了值却有解析不出的 token → Err（此前静默丢弃，全非法时等于不加谓词、
/// 返回全站订阅源，比报错危险）
fn parse_ids(s: Option<&str>) -> Result<Option<Vec<i32>>, String> {
    let s = match s {
        Some(v) => v.trim(),
        None => return Ok(None),
    };
    if s.is_empty() {
        return Ok(None);
    }
    let mut ids: Vec<i32> = Vec::new();
    for p in s.split(',') {
        let p = p.trim();
        if p.is_empty() {
            continue;
        }
        match p.parse::<i32>() {
            Ok(v) => {
                if !ids.contains(&v) {
                    ids.push(v);
                }
            }
            Err(_) => return Err(format!("参数非法：{p}")),
        }
    }
    Ok(if ids.is_empty() { None } else { Some(ids) })
}

fn format_size(b: i64) -> String {
    let gb = b as f64 / 1024f64.powi(3);
    if gb >= 1.0 {
        format!("{gb:.2} GB")
    } else {
        format!("{:.0} MB", b as f64 / 1024f64.powi(2))
    }
}

#[cfg(test)]
mod tests {
    use super::parse_ids;

    #[test]
    fn parse_ids_multi_and_invalid() {
        assert_eq!(parse_ids(Some("1,2, 3")), Ok(Some(vec![1, 2, 3])));
        assert_eq!(parse_ids(Some("5")), Ok(Some(vec![5])));
        assert_eq!(parse_ids(Some("")), Ok(None));
        assert_eq!(parse_ids(None), Ok(None));
        // 给了值却解析不出来：报错，不再退化成「不过滤 = 返回全站」
        assert!(parse_ids(Some("x,y")).is_err());
        assert!(parse_ids(Some("1,abc")).is_err());
        // 重复值合并，避免生成重复谓词
        assert_eq!(parse_ids(Some("2,2")), Ok(Some(vec![2])));
        // 旧字段兼容由 handler 单独处理
    }
}
