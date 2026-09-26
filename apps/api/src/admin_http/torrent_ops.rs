use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;
use super::user_list::{default_page, default_per_page};

// ============ 第五轮：种子操作记录（参考站 torrent-operation-logs 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct TorrentOpRow {
    id: i64,
    torrent_id: i64,
    torrent_name: Option<String>,
    operator_name: Option<String>,
    action: String,
    detail: Option<serde_json::Value>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct TorrentOpQ {
    #[serde(default)]
    torrent_id: Option<i64>,
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_per_page")]
    per_page: i64,
}

#[get("/admin/torrent-ops")]
async fn torrent_op_logs(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<TorrentOpQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let (rows, total): (Vec<TorrentOpRow>, i64) = if let Some(tid) =
        q.torrent_id
    {
        let rows: Vec<TorrentOpRow> = sqlx::query_as(
            r#"SELECT l.id, l.torrent_id, t.name AS torrent_name,
                      u.username AS operator_name,
                      l.action, l.detail, l.created_at
               FROM torrent_operation_logs l
               LEFT JOIN torrents t ON t.id = l.torrent_id
               LEFT JOIN users u ON u.id = l.operator_id
               WHERE l.torrent_id = $1
               ORDER BY l.id DESC LIMIT $2 OFFSET $3"#,
        )
        .bind(tid)
        .bind(crate::dto::page_window(q.page, q.per_page).1)
        .bind(crate::dto::page_window(q.page, q.per_page).0)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM torrent_operation_logs WHERE torrent_id = $1",
        )
        .bind(tid)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        (rows, total)
    } else {
        let rows: Vec<TorrentOpRow> = sqlx::query_as(
            r#"SELECT l.id, l.torrent_id, t.name AS torrent_name,
                      u.username AS operator_name,
                      l.action, l.detail, l.created_at
               FROM torrent_operation_logs l
               LEFT JOIN torrents t ON t.id = l.torrent_id
               LEFT JOIN users u ON u.id = l.operator_id
               ORDER BY l.id DESC LIMIT $1 OFFSET $2"#,
        )
        .bind(crate::dto::page_window(q.page, q.per_page).1)
        .bind(crate::dto::page_window(q.page, q.per_page).0)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let total: i64 =
            sqlx::query_scalar("SELECT count(*) FROM torrent_operation_logs")
                .fetch_one(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        (rows, total)
    };
    Ok(ok(serde_json::json!({
        "rows": rows, "total": total,
        "page": q.page.max(1), "per_page": q.per_page,
    })))
}
