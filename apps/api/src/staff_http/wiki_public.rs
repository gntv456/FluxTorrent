//! Wiki 公开读（0359，从 wiki.rs 拆出——300 行门禁）：
//! GET /wiki 目录 + GET /wiki/{slug} 正文。
//!
//! 与 custom_pages 的关键差异：正文是 BBCode——前端 renderBBCode 渲染成
//! React 节点，天然免疫 XSS，出站**不做** ammonia 消毒（custom_pages 是
//! 管理员 HTML、注水注定的，才需要那道防线）。

use actix_web::{get, web, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

// ============ 公开读 ============

/// GET /wiki：目录（visible 页清单；模块开关由网关 /api/v1/wiki 前缀拦截）
#[get("/wiki")]
pub async fn wiki_public_index(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, slug, title FROM wiki_pages \
         WHERE visible ORDER BY sort, id LIMIT 500",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    // 必须投影成对象：`Vec<tuple>` 直接序列化是 JSON 数组的数组，而 web 侧
    // （app/(main)/wiki/[[...slug]]/page.tsx:19）按 {id,slug,title} 取字段 ⇒
    // 元组形态下目录页每条的 slug/title 都是 undefined（批十一 /friends 同型）
    Ok(ok(rows
        .into_iter()
        .map(|(id, slug, title)| {
            serde_json::json!({ "id": id, "slug": slug, "title": title })
        })
        .collect::<Vec<_>>()))
}

/// GET /wiki/{slug}：正文（visible=true 才返回，否则 404 信封）。
/// 注意：不做 HTML 消毒——正文是 BBCode，前端 renderBBCode 渲染成
/// React 节点（无 innerHTML 注水点），XSS 面与论坛帖同源而非 custom_pages。
#[get("/wiki/{slug}")]
pub async fn wiki_public_page(
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let slug = path.into_inner();
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT title, body, updated_at::text FROM wiki_pages \
         WHERE slug = $1 AND visible",
    )
    .bind(&slug)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(internal)?;
    let Some((title, body, updated)) = row else {
        return Err(DomainError::NotFound(0));
    };
    Ok(ok(serde_json::json!({
        "slug": slug, "title": title,
        "body": body, "updated_at": updated,
    })))
}
