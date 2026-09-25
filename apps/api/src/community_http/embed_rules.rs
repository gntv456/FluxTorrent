//! 论坛视频内嵌 V1（0189）：embed 规则的公开只读端点。
//!
//! 渲染侧安全模型：正文存 Markdown 原文，前端渲染层拿本端点的启用规则
//! 做「URL 正则匹配 → 模板生成 src → embed_origin 前缀二次校验」，
//! 未命中规则的 URL 一律降级为普通链接（fail-closed）。
//! 规则的写入口在 staff_http / admin_http（FORUMS_MANAGE）。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Serialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(sqlx::FromRow, Serialize)]
pub struct EmbedRulePublic {
    pub id: i64,
    pub provider: String,
    pub name_zh: String,
    pub url_pattern: String,
    pub embed_template: String,
    pub embed_origin: String,
    pub render_kind: String,
    pub aspect: String,
    pub extra_params: Option<String>,
}

/// GET /forums/embed-rules：启用规则（总开关 forum_video_embed=no 时返回空，
/// 前端据此把全部视频语法降级为链接）。
#[get("/forums/embed-rules")]
async fn forum_embed_rules(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = require_auth(&req, &state).await?;
    let on: bool = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'forum_video_embed'), 'yes') = 'yes'",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let rows: Vec<EmbedRulePublic> = if on {
        sqlx::query_as(
            "SELECT id, provider, name_zh, url_pattern, embed_template, \
             embed_origin, render_kind, aspect, extra_params \
             FROM video_embed_rules WHERE enabled ORDER BY sort, id",
        )
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    } else {
        Vec::new()
    };
    Ok(ok(rows))
}
