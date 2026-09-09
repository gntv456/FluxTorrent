//! M08 RSS 订阅：getrss 等价 —— 按 passkey 鉴权的个性化 RSS 2.0 输出。
//!
//! NexusPHP 口径：用户专属 token（我们复用 passkey）+ 可选过滤参数；
//! 刷流工具（RSS 阅读器/下载器）凭 URL 自动拉新种。

use actix_web::{get, web, HttpResponse};
use chrono::{DateTime, Utc};

use crate::state::AppState;

pub fn mount_rss(scope: actix_web::Scope) -> actix_web::Scope {
    scope.service(rss_feed)
}

#[derive(sqlx::FromRow)]
struct RssRow {
    id: i64,
    name: String,
    small_descr: Option<String>,
    size: i64,
    created_at: DateTime<Utc>,
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// GET /rss/{passkey}?category=&official=
/// passkey 即用户身份（BEP3 同源凭证，泄露可自助 rotate —— 与 tracker 一致的暴露面）
#[get("/rss/{passkey}")]
async fn rss_feed(
    path: web::Path<String>,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<RssQuery>,
) -> HttpResponse {
    let passkey = path.into_inner();
    if passkey.len() != 32 {
        return HttpResponse::BadRequest().body("invalid passkey");
    }
    let user: Option<i64> =
        sqlx::query_scalar("SELECT id FROM users WHERE passkey = $1 AND status < 2")
            .bind(&passkey)
            .fetch_optional(&state.repo.db)
            .await
            .unwrap_or(None);
    if user.is_none() {
        return HttpResponse::NotFound().body("unknown passkey");
    }

    let rows: Vec<RssRow> = sqlx::query_as(
        "SELECT id, name, small_descr, size, created_at FROM torrents \
         WHERE approval_status = 1 \
           AND ($1::int IS NULL OR category_id = $1) \
           AND ($2::bool IS NULL OR official_tag = $2) \
         ORDER BY id DESC LIMIT 50",
    )
    .bind(q.category)
    .bind(q.official)
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();

    let base = std::env::var("PUBLIC_SITE_URL").unwrap_or_else(|_| "http://localhost:3000".into());
    let mut items = String::new();
    for r in &rows {
        let descr = r.small_descr.clone().unwrap_or_default();
        items.push_str(&format!(
            "<item><title>{}</title><link>{}/torrent/{}</link>\
             <guid isPermaLink=\"true\">{}/torrent/{}</guid>\
             <pubDate>{}</pubDate><description>{} · {}</description></item>",
            xml_escape(&r.name),
            base,
            r.id,
            base,
            r.id,
            r.created_at.format("%a, %d %b %Y %H:%M:%S GMT"),
            xml_escape(&descr),
            format_size(r.size),
        ));
    }
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <rss version=\"2.0\"><channel>\
         <title>FluxTorrent · 好学 最新种子</title>\
         <link>{}</link><description>教育资源私有种子社区 RSS</description>\
         <ttl>15</ttl>{}</channel></rss>",
        base, items
    );
    HttpResponse::Ok()
        .content_type("application/rss+xml; charset=utf-8")
        .body(xml)
}

#[derive(serde::Deserialize)]
struct RssQuery {
    category: Option<i32>,
    official: Option<bool>,
}

fn format_size(b: i64) -> String {
    let gb = b as f64 / 1024f64.powi(3);
    if gb >= 1.0 {
        format!("{gb:.2} GB")
    } else {
        format!("{:.0} MB", b as f64 / 1024f64.powi(2))
    }
}
