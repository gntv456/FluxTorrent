//! 帮助中心公开端点（0274）。
//!
//! 与 staff_http::custom_pages 的后台 CRUD 分开放：本模块只读、只管
//! 「列出帮助目录」，避免把 300 行门禁文件继续撑大。
//!
//! 可见性判据与 /pages/{slug} 完全一致（visible + 模块开启）——目录页
//! 不列出用户点进去会 404 的条目。

use actix_web::{get, web, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

#[derive(serde::Serialize)]
pub struct HelpDoc {
    pub slug: String,
    pub title: String,
    pub group: String,
    pub sort: i32,
}

/// GET /help-docs：帮助中心目录（doc_group 非空的可见页）。
/// 组内按 doc_sort 升序（NULL 回落 sort），组间按分组名稳定排序。
#[get("/help-docs")]
pub async fn help_docs_list(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let rows: Vec<(String, String, Option<String>, Option<i32>, i32)> =
        sqlx::query_as(&format!(
            "SELECT slug, title, doc_group, doc_sort, sort FROM custom_pages \
             WHERE visible AND doc_group IS NOT NULL AND {} \
             ORDER BY doc_group, COALESCE(doc_sort, sort), id LIMIT 500",
            crate::modules::module_on_sql("custom_pages")
        ))
        .fetch_all(&state.repo.db)
        .await
        .map_err(internal)?;

    let docs = rows
        .into_iter()
        .map(|(slug, title, group, dsort, sort)| HelpDoc {
            slug,
            title,
            group: group.unwrap_or_default(),
            sort: dsort.unwrap_or(sort),
        })
        .collect::<Vec<_>>();

    Ok(ok(docs))
}
