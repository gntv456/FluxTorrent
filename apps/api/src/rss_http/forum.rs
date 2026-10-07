//! 论坛 RSS（Phase3 窄版）：按版块的新帖 feed，接 TG Bot / RSS 阅读器。
//!
//! 口径与种子 RSS 完全同源：passkey 鉴权（泄露可自助 rotate，暴露面与 tracker 一致）、
//! XML 管线与频道名（SITENAME）复用。默认只推「站务公告」版（id=1）——主打通告转发场景，
//! 想订阅别的版块自己加 ?forums=1,6。权限：逐条复刻 can_read（minclassread / 版主），
//! passkey 主人无权读的版块不出现在 feed 里（与关注流同一越权口径）。

use actix_web::{get, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};

use super::{parse_ids, xml_escape};
use crate::state::AppState;

#[derive(serde::Deserialize)]
struct ForumRssQuery {
    /// 版块多选（逗号分隔）；缺省 = 仅站务公告版（id 最小的公告定位：name LIKE '公告%'）
    forums: Option<String>,
    showrows: Option<i64>,
}

#[get("/rss/forum/{passkey}")]
async fn forum_rss_feed(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ForumRssQuery>,
) -> HttpResponse {
    let passkey = path.into_inner();
    // 四轮审计 M3（2026-10-07）：0267 限流补丁漏了论坛 RSS——主 RSS 有
    // valid_passkey+双维限流，这里只查长度，匿名换随机 passkey 即无限
    // 打 users 索引。对齐主 RSS 同款两闸。
    if !crate::compat_http::valid_passkey(&passkey) {
        return HttpResponse::BadRequest().body("invalid passkey");
    }
    if crate::compat_http::limit_passkey(&state, &req, "rssforum", &passkey, 30)
        .await
        .is_err()
    {
        return HttpResponse::TooManyRequests()
            .insert_header(("retry-after", "60"))
            .body("rate limited");
    }
    let user: Option<(i64, i32)> = sqlx::query_as(
        "SELECT id, class_id FROM users WHERE passkey = $1 AND status < 2",
    )
    .bind(&passkey)
    .fetch_optional(&state.repo.db)
    .await
    .unwrap_or(None);
    let Some((uid, class_id)) = user else {
        return HttpResponse::NotFound().body("unknown passkey");
    };
    // forums 模块关闭 → 订阅源整体下线（二审 G8：RSS 出口不受网关覆盖，
    // 必须在 handler 内判定；404 语义与刷流工具的重试退避兼容）
    if !state.module_enabled("forums").await {
        return HttpResponse::NotFound().body("forums disabled");
    };
    // 版块筛选：缺省回落「公告版」（name LIKE '公告%' 的最小 id；找不到则 id=1）。
    // 非法值直接 400——静默回落到公告版会让订阅方以为自己筛的是别的版块
    let wanted = match parse_ids(q.forums.as_deref()) {
        Ok(v) => v,
        Err(msg) => return HttpResponse::BadRequest().body(msg),
    };
    let fids: Vec<i64> = match wanted {
        Some(v) => v.into_iter().map(|x| x as i64).collect(),
        None => vec![sqlx::query_scalar(
            "SELECT COALESCE((SELECT min(id) FROM forums WHERE \
             name LIKE '公告%'), 1)",
        )
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(1)],
    };
    let rows: Vec<(i64, String, i64, String, Option<String>, DateTime<Utc>)> =
        sqlx::query_as(
            "SELECT t.id, t.title, f.id, f.name, u.username, t.created_at \
             FROM topics t \
             JOIN forums f ON f.id = t.forum_id \
             LEFT JOIN users u ON u.id = t.user_id \
             WHERE f.id = ANY($1) \
               AND (f.minclassread <= $2 OR EXISTS (SELECT 1 FROM forum_mods fm WHERE fm.forum_id = f.id AND fm.user_id = $3)) \
             ORDER BY t.id DESC LIMIT $4",
        )
        .bind(&fids)
        .bind(class_id)
        .bind(uid)
        .bind(q.showrows.unwrap_or(50).clamp(1, 200))
        .fetch_all(&state.repo.db)
        .await
        .unwrap_or_default();
    let base = std::env::var("PUBLIC_SITE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| {
            req.headers()
                .get("host")
                .and_then(|v| v.to_str().ok())
                .map(str::trim)
                .filter(|h| crate::compat_http::aliases::sane_host(h))
                .map(|h| format!("http://{h}"))
                .unwrap_or_else(|| "http://localhost:3000".into())
        });
    let mut items = String::new();
    for (tid, title, _fid, fname, author, at) in &rows {
        items.push_str(&format!(
            "<item><title>[{}] {}</title><link>{}/forums/topic/{}</link>\
             <guid isPermaLink=\"true\">{}/forums/topic/{}</guid>\
             <pubDate>{}</pubDate><description>{}</description></item>",
            xml_escape(fname),
            xml_escape(title),
            base,
            tid,
            base,
            tid,
            at.format("%a, %d %b %Y %H:%M:%S GMT"),
            xml_escape(author.as_deref().unwrap_or("匿名")),
        ));
    }
    let channel_title: String = sqlx::query_scalar(
        "SELECT COALESCE(NULLIF((SELECT value FROM site_settings WHERE name = 'SITENAME'), ''), \
                         NULLIF((SELECT value FROM site_settings WHERE name = 'site_name'), ''), \
                         'FluxTorrent')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_else(|_| "FluxTorrent".into());
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <rss version=\"2.0\"><channel>\
         <title>{} 论坛新帖</title>\
         <link>{}/forums</link><description>论坛版块 RSS</description>\
         <ttl>15</ttl>{}</channel></rss>",
        xml_escape(&channel_title),
        base,
        items
    );
    HttpResponse::Ok()
        .content_type("application/rss+xml; charset=utf-8")
        .body(xml)
}
