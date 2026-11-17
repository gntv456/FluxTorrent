//! Wiki 公开读（0359，从 wiki.rs 拆出——300 行门禁）：
//! GET /wiki 目录 + GET /wiki/{slug} 正文 + GET /wiki/search 检索。
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

/// GET /wiki：目录（visible 页清单；模块开关由网关 /api/v1/wiki 前缀拦截）。
/// 0495 起带 section（分组目录卡片用）+ updated_at（组内排序/展示）。
#[get("/wiki")]
pub async fn wiki_public_index(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let rows: Vec<(i64, String, String, String, String)> = sqlx::query_as(
        "SELECT id, slug, title, section, updated_at::text \
         FROM wiki_pages WHERE visible ORDER BY sort, id LIMIT 500",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    // 必须投影成对象：`Vec<tuple>` 直接序列化是 JSON 数组的数组，而 web 侧
    // （app/(main)/wiki/[[...slug]]/page.tsx）按字段名取值 ⇒
    // 元组形态下目录页每条的 slug/title 都是 undefined（批十一 /friends 同型）
    Ok(ok(rows
        .into_iter()
        .map(|(id, slug, title, section, updated)| {
            serde_json::json!({
                "id": id, "slug": slug, "title": title,
                "section": section, "updated_at": updated,
            })
        })
        .collect::<Vec<_>>()))
}

/// GET /wiki/search?q=关键字：标题/正文 ILIKE 检索（visible 限定）。
///
/// 排序口径：标题命中排前（入门者通常凭页名搜索），同为标题命中按 sort；
/// 每页上限 20 条——wiki 是站长手写的小语料库，不做分页只做封顶，
/// 免得一条宽匹配把目录页撑成长卷。q 首尾空白会被剥掉，空串回空数组
/// （前端目录页的即时过滤不需要打到这条——它拿全量目录本地筛）。
#[get("/wiki/search")]
pub async fn wiki_public_search(
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<WikiSearchQuery>,
) -> DomainResult<HttpResponse> {
    let needle = q.q.trim().to_string();
    if needle.is_empty() {
        return Ok(ok(serde_json::json!({ "items": [], "count": 0 })));
    }
    // ILIKE 的 %/_ 是通配符：用户输入里的字面 % 会放大匹配面，
    // 转义后按字面处理（PG 口径：ESCAPE '\'）
    let esc = needle
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let pat = format!("%{esc}%");
    let rows: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT id, slug, title, section FROM wiki_pages \
         WHERE visible AND (title ILIKE $1 OR body ILIKE $1) \
         ORDER BY (title ILIKE $1) DESC, sort, id LIMIT 20",
    )
    .bind(&pat)
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    let count = rows.len();
    Ok(ok(serde_json::json!({
        "items": rows.into_iter().map(|(id, slug, title, section)| {
            serde_json::json!({
                "id": id, "slug": slug, "title": title, "section": section,
            })
        }).collect::<Vec<_>>(),
        "count": count,
    })))
}

#[derive(serde::Deserialize)]
pub(crate) struct WikiSearchQuery {
    q: String,
}

/// GET /wiki/{slug}：正文（visible=true 才返回，否则 404 信封）。
/// 注意：不做 HTML 消毒——正文是 BBCode，前端 renderBBCode 渲染成
/// React 节点（无 innerHTML 注水点），XSS 面与论坛帖同源而非 custom_pages。
/// 0495 起带 section + updated_at（正文页左栏同组导航需要）。
#[get("/wiki/{slug}")]
pub async fn wiki_public_page(
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let slug = path.into_inner();
    let row: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT title, body, section, updated_at::text FROM wiki_pages \
         WHERE slug = $1 AND visible",
    )
    .bind(&slug)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(internal)?;
    let Some((title, body, section, updated)) = row else {
        return Err(DomainError::NotFound(0));
    };
    Ok(ok(serde_json::json!({
        "slug": slug, "title": title, "body": body,
        "section": section, "updated_at": updated,
    })))
}
