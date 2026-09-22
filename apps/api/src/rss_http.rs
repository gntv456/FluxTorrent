//! M08 RSS 订阅：getrss 等价 —— 按 passkey 鉴权的个性化 RSS 2.0 输出。
//!
//! NexusPHP 口径：用户专属 token（我们复用 passkey）+ 可选过滤参数；
//! 刷流工具（RSS 阅读器/下载器）凭 URL 自动拉新种。
//! 参数对齐参考站 getrss.php 的常用子集：分类多选/媒介多选/官种/关键字/条数/标题格式/付费。

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
    if passkey.len() != 32 {
        return HttpResponse::BadRequest().body("invalid passkey");
    }
    let user: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE passkey = $1 AND status < 2",
    )
    .bind(&passkey)
    .fetch_optional(&state.repo.db)
    .await
    .unwrap_or(None);
    if user.is_none() {
        return HttpResponse::NotFound().body("unknown passkey");
    }

    // 多选分类/媒介：逗号分隔 → 数组（空 = 不过滤）；兼容旧版单值 category
    let categories = parse_ids(q.categories.as_deref())
        .or_else(|| q.category.map(|c| vec![c]));
    let mediums = parse_ids(q.mediums.as_deref());
    // paid=1 → 仅免费促销种（当前生效的 torrent 级或 scope 级 free/x2free；修复前参数被静默忽略）
    let free_only = q.paid == Some(1);
    // 促销口径与列表/筛选谓词共用同一份实现（crate::torrents::promo）：
    // 此前 RSS 自己内联了一份命中条件 + 优先级 CASE，与主列表口径会各自漂移。
    let promo_lateral = crate::torrents::promo::lateral_latest();
    let free_clause =
        crate::torrents::promo::exists_clause(Some("'free','x2free'"));
    let sql = format!(
        "SELECT t.id, t.name, t.small_descr, t.size, t.created_at, t.official_tag, \
                pr.promotion, \
                u.username AS owner_name \
         FROM torrents t LEFT JOIN users u ON u.id = t.owner_id \
         {promo_lateral} \
         WHERE t.approval_status = 1 \
           AND ($1::int[] IS NULL OR t.category_id = ANY($1)) \
           AND ($2::int[] IS NULL OR \
                t.medium_id = ANY($2) \
                OR EXISTS (SELECT 1 FROM torrent_sections ts \
                           WHERE ts.torrent_id = t.id AND ts.kind = 'media' AND ts.dict_id = ANY($2))) \
           AND ($3::bool IS NULL OR t.official_tag = $3) \
           AND ($4::text IS NULL OR t.name ILIKE '%' || $4 || '%') \
           AND (NOT $6::bool OR {free_clause}) \
         ORDER BY t.id DESC LIMIT $5"
    );
    let rows: Vec<RssRow> = sqlx::query_as(&sql)
        .bind(categories.as_deref())
        .bind(mediums.as_deref())
        .bind(q.official)
        .bind(q.search.as_deref().filter(|s| !s.is_empty()))
        .bind(q.showrows.unwrap_or(50).clamp(1, 200))
        .bind(free_only)
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
    // 标题格式：linktype=dl（默认）[促销] [官种] 标题 [副标题] 大小 发布者；linktype=page 仅标题。
    // 促销标记（刷流时效性关键，NP 官方插件口径）：用户自购/站方挂的 Free 与 2xFree
    // 必须在 RSS 标题第一时间可见——刷流器按标题关键词过滤，标记缺失 = 免费信息传不到
    // （NP 二改站的常见缺陷）。标记格式与列表角标一致：Free / 2xFree / 50% / 2x / 2x50%。
    let promo_tag = |p: &Option<String>| -> String {
        match p.as_deref() {
            Some("free") => "[Free] ".into(),
            Some("x2free") => "[2xFree] ".into(),
            Some("half") => "[50%] ".into(),
            Some("x2half") => "[2x50%] ".into(),
            Some("x2") => "[2x] ".into(),
            Some("p30") => "[30%] ".into(),
            _ => String::new(),
        }
    };
    let verbose = q.linktype.as_deref() != Some("page");
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
        items.push_str(&format!(
            "<item><title>{}</title><link>{}/torrent/{}</link>\
             <guid isPermaLink=\"true\">{}/torrent/{}</guid>\
             <pubDate>{}</pubDate><description>{} · {}</description></item>",
            xml_escape(&title),
            base,
            r.id,
            base,
            r.id,
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
    /// 0=全部 1=仅免费（当前生效的 free/x2free 促销种；与好学 paid 口径对齐）
    paid: Option<i32>,
}

fn parse_ids(s: Option<&str>) -> Option<Vec<i32>> {
    let s = s?.trim();
    if s.is_empty() {
        return None;
    }
    let ids: Vec<i32> = s
        .split(',')
        .filter_map(|p| p.trim().parse::<i32>().ok())
        .collect();
    if ids.is_empty() {
        None
    } else {
        Some(ids)
    }
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
        assert_eq!(parse_ids(Some("1,2, 3")), Some(vec![1, 2, 3]));
        assert_eq!(parse_ids(Some("5")), Some(vec![5]));
        assert_eq!(parse_ids(Some("")), None);
        assert_eq!(parse_ids(Some("x,y")), None);
        assert_eq!(parse_ids(None), None);
        // 旧字段兼容由 handler 单独处理
    }
}
