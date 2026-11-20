//! 公告管理 + 趣味盒 + 友情链接（news/fun/linksmanage 复刻）。
//! 从 http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, Responder};
use serde::Deserialize;

// auth 模块经 state.jwt 使用（0071 RS256 化后 http 层不再直接调用）

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::auth_infra::require_auth;

// ============ 公告管理（news.php 复刻：发布/编辑/删除，staff 专用） ============

/// GET /admin/news —— 后台公告列表（NEWS_MANAGE/ANNOUNCE_PUBLISH）。
/// 含未到点的定时待发条目（首页读侧只出已到点，后台必须能看全量，
/// 否则定时公告发布后后台不可见、不可改）。published_at 供前端标记。
#[get("/admin/news")]
pub async fn news_admin_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_any_perm(
        &state,
        &auth,
        &[
            crate::authz::perm::NEWS_MANAGE,
            crate::authz::perm::ANNOUNCE_PUBLISH,
        ],
    )
    .await?;
    #[derive(sqlx::FromRow, serde::Serialize)]
    struct AdminNewsRow {
        id: i32,
        title: String,
        badge: String,
        created_at: chrono::DateTime<chrono::Utc>,
        published_at: Option<chrono::DateTime<chrono::Utc>>,
    }
    let rows: Vec<AdminNewsRow> = sqlx::query_as(
        "SELECT id, title, badge, created_at, published_at \
         FROM announcements ORDER BY id DESC LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct NewsBody {
    title: String,
    body: String,
    #[serde(default)]
    badge: String,
    /// 定时发布（0505 运营体检批）：ISO8601；缺省/空=立即（published_at=now）。
    /// 未来时间=预热待发，读侧到点才可见。
    #[serde(default)]
    publish_at: Option<String>,
}

#[post("/admin/news")]
pub async fn news_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<NewsBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_any_perm(
        &state,
        &auth,
        &[
            crate::authz::perm::NEWS_MANAGE,
            crate::authz::perm::ANNOUNCE_PUBLISH,
        ],
    )
    .await?;
    if body.title.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("标题和正文不能为空".into()));
    }
    // 定时发布（0505）：解析失败直接 422，不静默吞成立即发布
    let publish_at = match body.publish_at.as_deref() {
        None | Some("") => Some(chrono::Utc::now()),
        Some(s) => Some(
            chrono::DateTime::parse_from_rfc3339(s)
                .map_err(|_| {
                    DomainError::Validation(
                        "publish_at 须为 ISO8601（如 2026-11-21T09:00:00Z）"
                            .into(),
                    )
                })?
                .with_timezone(&chrono::Utc),
        ),
    };
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO announcements (title, body, badge, author_id, \
         sort, published_at) VALUES ($1, $2, $3, $4, (SELECT \
         COALESCE(max(sort),0)+10 FROM announcements), $5) RETURNING id",
    )
    .bind(body.title.trim())
    .bind(&body.body)
    .bind(if body.badge.trim().is_empty() {
        "公告"
    } else {
        body.badge.trim()
    })
    .bind(auth.id)
    .bind(publish_at)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "news_create", None).await;
    // 首页共享缓存写失效（0152）：公告进 /home 共享段
    super::home::invalidate_home_cache(&state).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/news/{id}")]
pub async fn news_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<NewsBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_any_perm(
        &state,
        &auth,
        &[
            crate::authz::perm::NEWS_MANAGE,
            crate::authz::perm::ANNOUNCE_PUBLISH,
        ],
    )
    .await?;
    let updated = sqlx::query(
        "UPDATE announcements SET title = $2, body = $3, \
         badge = $4, \
         published_at = COALESCE($5, published_at) WHERE id = $1",
    )
    .bind(*path)
    .bind(body.title.trim())
    .bind(&body.body)
    .bind(if body.badge.trim().is_empty() {
        "公告"
    } else {
        body.badge.trim()
    })
    .bind(body.publish_at.as_deref().and_then(|s| {
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|t| t.with_timezone(&chrono::Utc))
    }))
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::NotFound(*path));
    }
    state.repo.audit(Some(auth.id), "news_update", None).await;
    super::home::invalidate_home_cache(&state).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/news/{id}")]
pub async fn news_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_any_perm(
        &state,
        &auth,
        &[
            crate::authz::perm::NEWS_MANAGE,
            crate::authz::perm::ANNOUNCE_PUBLISH,
        ],
    )
    .await?;
    let deleted = sqlx::query("DELETE FROM announcements WHERE id = $1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if deleted.rows_affected() == 0 {
        return Err(DomainError::NotFound(*path));
    }
    state.repo.audit(Some(auth.id), "news_delete", None).await;
    super::home::invalidate_home_cache(&state).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}
