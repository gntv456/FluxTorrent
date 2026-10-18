//! 自定义页的公开读侧（批五十三按域拆出）：`/public-pages`（sitemap 来源）
//! 与 `/pages/{slug}`（前台渲染，出站 ammonia 消毒）。
//!
//! 与 `custom_pages.rs`（后台 CRUD）分开放：那件已贴 300 行门禁，而本件的两个
//! handler 只读、不碰鉴权，混在一起会让「可见性判据改一处漏一处」更难发现——
//! 两处的判据必须一致（visible + 模块开启），本件是唯一实现处。

use actix_web::{get, web, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

/// GET /public-pages：可被前台访问的自定义页清单（sitemap 用；无登录态）。
/// 与 /pages/{slug} 同一条可见性判据——挂了模块键且模块关闭的页面不出现在这里，
/// 免得爬虫拿到一个必然 404 的地址。
#[get("/public-pages")]
pub async fn custom_pages_public_list(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let rows: Vec<(String, String)> =
        sqlx::query_as::<_, (String, String)>(&format!(
            "SELECT slug, updated_at::date::text FROM custom_pages \
             WHERE visible AND {} ORDER BY sort, id LIMIT 500",
            crate::modules::module_on_sql("custom_pages")
        ))
        .fetch_all(&state.repo.db)
        .await
        .map_err(internal)?;
    Ok(ok(
        rows.into_iter()
            .map(|(slug, updated)| {
                serde_json::json!({ "slug": slug, "updated_at": updated })
            })
            .collect::<Vec<_>>(),
    ))
}

/// GET /pages/{slug}：消毒后的页面内容（visible=false 或不存在 → 404 信封）
#[get("/pages/{slug}")]
pub async fn custom_page_public(
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let slug = path.into_inner();
    let row: Option<(String, String, String)> =
        sqlx::query_as::<_, (String, String, String)>(&format!(
            "SELECT title, body, updated_at::text FROM custom_pages \
             WHERE slug = $1 AND visible AND {}",
            crate::modules::module_on_sql("custom_pages")
        ))
        .bind(&slug)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(internal)?;
    let Some((title, body, updated)) = row else {
        return Err(DomainError::NotFound(0));
    };
    // 出站消毒：与公告**同一把尺**（http::home_news::html_clean）——剥
    // script/事件属性/javascript:，并放行 iframe/video/source 且 src 过
    // video_embed_rules 的 enabled 集合（规则空时一律剥 src，fail-closed）。
    // 原先这里只跑 ammonia 默认集，iframe/video 整块被吞：站长贴进页面的
    // 视频在后台预览看得到、访客端静默消失（P-341）。
    let origins =
        crate::http::home_news::video_embed_origins(&state.repo.db).await;
    let safe = crate::http::home_news::html_clean(&body, &origins);
    Ok(ok(serde_json::json!({
        "slug": slug, "title": title,
        "body": safe, "updated_at": updated,
    })))
}
