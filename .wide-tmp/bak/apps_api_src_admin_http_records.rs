use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;
use super::user_list::{default_page, default_per_page};

// ============ 第五轮：记录查询（参考站 火花记录/种子购买/登录记录 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct SparkLogRow {
    id: i64,
    username: String,
    amount: i64,
    kind: String,
    balance_after: Option<i64>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct RecordQ {
    #[serde(default)]
    q: String,
    /// 用户 ID 精确过滤（详情页关联 tab 用）
    #[serde(default)]
    user_id: Option<i64>,
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_per_page")]
    per_page: i64,
}

#[get("/admin/spark-logs")]
async fn admin_spark_logs(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<RecordQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let pattern = crate::http::like_pattern(&q.q);
    let rows: Vec<SparkLogRow> = sqlx::query_as(
        r#"SELECT l.id, u.username, l.amount, l.kind, l.balance_after, l.created_at
           FROM spark_ledger l JOIN users u ON u.id = l.user_id
           WHERE u.username ILIKE $1 AND ($2::bigint IS NULL OR l.user_id = $2)
           ORDER BY l.created_at DESC, l.id DESC LIMIT $3 OFFSET $4"#,
    )
    .bind(&pattern)
    .bind(q.user_id)
    .bind(crate::dto::page_window(q.page, q.per_page).1)
    .bind(crate::dto::page_window(q.page, q.per_page).0)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM spark_ledger l JOIN users u ON u.id = l.user_id \
         WHERE u.username ILIKE $1 AND ($2::bigint IS NULL OR l.user_id = $2)",
    )
    .bind(&pattern)
    .bind(q.user_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "page": q.page.max(1), "per_page": q.per_page, "total": total }),
    ))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct TorrentBuyRow {
    id: i64,
    username: String,
    kind: String,
    ref_id: Option<i64>,
    amount: i64,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/torrent-buys")]
async fn admin_torrent_buys(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<RecordQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let pattern = crate::http::like_pattern(&q.q);
    let rows: Vec<TorrentBuyRow> = sqlx::query_as(
        r#"SELECT l.id, u.username, l.kind, l.ref_id, l.amount, l.created_at
           FROM spark_ledger l JOIN users u ON u.id = l.user_id
           WHERE l.kind IN ('torrent_buy', 'buy_torrent', 'token_buy') AND u.username ILIKE $1
           ORDER BY l.created_at DESC, l.id DESC LIMIT $2 OFFSET $3"#,
    )
    .bind(pattern)
    .bind(crate::dto::page_window(q.page, q.per_page).1)
    .bind(crate::dto::page_window(q.page, q.per_page).0)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "rows": rows, "page": q.page.max(1), "per_page": q.per_page }),
    ))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct LoginLogRow {
    id: i64,
    username: String,
    ip: Option<String>,
    ok: bool,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/login-logs")]
async fn admin_login_logs(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<RecordQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let pattern = crate::http::like_pattern(&q.q);
    let rows: Vec<LoginLogRow> = sqlx::query_as(
        r#"SELECT l.id, u.username, host(l.ip) AS ip, l.ok, l.created_at
           FROM login_events l JOIN users u ON u.id = l.user_id
           WHERE (u.username ILIKE $1 OR host(l.ip) ILIKE $1)
             AND ($2::bigint IS NULL OR l.user_id = $2)
           ORDER BY l.id DESC LIMIT $3 OFFSET $4"#,
    )
    .bind(&pattern)
    .bind(q.user_id)
    .bind(crate::dto::page_window(q.page, q.per_page).1)
    .bind(crate::dto::page_window(q.page, q.per_page).0)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // GeoIP（GeoLite2 离线库）：国家/城市随行返回；库缺失或内网 IP 时为 null
    let rows: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            let (iso, country, city) =
                r.ip.as_deref()
                    .map(crate::geo::lookup)
                    .unwrap_or((None, None, None));
            serde_json::json!({
                "id": r.id, "username": r.username, "ip": r.ip, "ok": r.ok,
                "country": iso, "country_name": country, "city": city,
                "created_at": r.created_at,
            })
        })
        .collect();
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM login_events l JOIN users u ON u.id = l.user_id \
         WHERE (u.username ILIKE $1 OR host(l.ip) ILIKE $1) AND ($2::bigint IS NULL OR l.user_id = $2)",
    )
    .bind(&pattern)
    .bind(q.user_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "rows": rows, "page": q.page.max(1), "per_page": q.per_page, "total": total }),
    ))
}
