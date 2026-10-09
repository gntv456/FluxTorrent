//! 元数据自动拉取（G7）：按 IMDb 链接补全简介与海报。
//! 从 ptgen.rs 按 300 行门禁拆出（纯搬运）；抓取内核仍是
//! `super::ptgen::fetch_meta`。

use actix_web::{post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 按 `imdb_id` 自动补全元数据（G7，对标 TMDB/IMDb 自动填充）。
/// owner 或 staff 触发；**只在原值为空时写入**（不覆盖手填内容）。
#[post("/torrents/{id}/auto-meta")]
pub async fn auto_meta(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let row: Option<(i64, Option<String>, Option<String>, serde_json::Value)> =
        sqlx::query_as(
            // imdb 有两个落点：`imdb_id` 列（审核/编辑口径）与
            // `media_info.imdb`（发种表单口径，见 UploadForm 注释）——两处都认。
            // media_info 可能为 NULL，必须 COALESCE 后再解成 Value（否则 500）。
            "SELECT owner_id, COALESCE(imdb_id, media_info->>'imdb'), descr, \
                    COALESCE(media_info, '{}'::jsonb) \
             FROM torrents WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((owner, imdb, descr, media_info)) = row else {
        return Err(DomainError::NotFound(id));
    };
    if owner != auth.id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let Some(imdb) = imdb.filter(|s| !s.trim().is_empty()) else {
        return Err(DomainError::Validation("该种未填 IMDb 链接".into()));
    };
    let (name, fetched, poster) =
        super::ptgen::fetch_meta(&state, imdb.trim()).await?;
    let mut filled: Vec<&str> = Vec::new();
    if descr.as_deref().unwrap_or("").trim().is_empty()
        && !fetched.trim().is_empty()
    {
        sqlx::query("UPDATE torrents SET descr = $1 WHERE id = $2")
            .bind(&fetched)
            .bind(id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        filled.push("descr");
    }
    let has_poster = media_info
        .get("poster")
        .and_then(|v| v.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if !has_poster {
        if let Some(p) = poster {
            sqlx::query(
                "UPDATE torrents SET media_info = \
                   COALESCE(media_info, '{}'::jsonb) \
                   || jsonb_build_object('poster', $1::text) \
                 WHERE id = $2",
            )
            .bind(&p)
            .bind(id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            filled.push("poster");
        }
    }
    Ok(ok(serde_json::json!({
        "id": id, "name": name, "filled": filled,
    })))
}
