//! GET /admin/export/{users|torrents}：运营数据导出（五路方案 P1-3.3）。
//! JSON 信封分页导出（每页 1000 行，翻页拿全量，不落盘）。
//! 权限：users 走 USER_ADJUST（同用户管理口径），torrents 走 TORRENT_MANAGE。

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

#[derive(Deserialize)]
struct ExportQ {
    page: Option<i64>,
}

/// 每页行数：导出口径固定 1000（分页翻页拿全量，防一次性大响应）。
const PER: i64 = 1000;

#[get("/admin/export/users")]
pub async fn admin_export_users(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ExportQ>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_ADJUST)
        .await?;
    let page = q.page.unwrap_or(1).clamp(1, 1_000_000);
    let rows: Vec<serde_json::Value> = sqlx::query_as::<
        _,
        (
            i64,
            String,
            Option<String>,
            i16,
            i64,
            i64,
            i32,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        "SELECT u.id, u.username, u.email, u.status, u.uploaded, \
             u.downloaded, u.class_id, u.created_at \
             FROM users u ORDER BY u.id LIMIT $1 OFFSET $2",
    )
    .bind(PER)
    .bind((page - 1) * PER)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .into_iter()
    .map(
        |(id, username, email, status, up, down, class_id, created)| {
            serde_json::json!({
                "id": id, "username": username,
                // 邮箱仅脱敏形态导出（运营报表口径，防导出文件外泄即泄邮箱）
                "email": email.map(|e| {
                    let (l, r) = e.split_once('@').unwrap_or((e.as_str(), ""));
                    format!("{}***@{}", &l[..l.len().min(2)], r)
                }),
                "status": status, "class_id": class_id,
                "uploaded": up, "downloaded": down,
                "created_at": created.to_rfc3339(),
            })
        },
    )
    .collect();
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "kind": "users", "items": rows, "total": total,
        "page": page, "per": PER,
    })))
}

#[get("/admin/export/torrents")]
pub async fn admin_export_torrents(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ExportQ>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::TORRENT_MANAGE,
    )
    .await?;
    let page = q.page.unwrap_or(1).clamp(1, 1_000_000);
    let rows: Vec<serde_json::Value> = sqlx::query_as::<
        _,
        (i64, String, i64, i64, i32, chrono::DateTime<chrono::Utc>),
    >(
        "SELECT t.id, t.name, t.owner_id, t.size, t.seeders, t.created_at \
             FROM torrents t ORDER BY t.id LIMIT $1 OFFSET $2",
    )
    .bind(PER)
    .bind((page - 1) * PER)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .into_iter()
    .map(|(id, name, owner_id, size, seeders, created)| {
        serde_json::json!({
            "id": id, "name": name, "owner_id": owner_id,
            "size": size, "seeders": seeders,
            "created_at": created.to_rfc3339(),
        })
    })
    .collect();
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM torrents")
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "kind": "torrents", "items": rows, "total": total,
        "page": page, "per": PER,
    })))
}
