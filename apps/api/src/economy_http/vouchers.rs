//! 免费券/中性券（0073 Gazelle FL token 口径）。
//! 从 economy_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 经济路由（挂载进主 /api/v1 scope，单 scope 避免遮蔽）

// ============ 免费券/中性券（0073，Gazelle FL token 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct VoucherRow {
    id: i64,
    kind: String,
    source: String,
    granted_at: chrono::DateTime<chrono::Utc>,
    expires_at: chrono::DateTime<chrono::Utc>,
    used_torrent_id: Option<i64>,
    used_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 我的券库存（含已用/过期历史，前端按状态分组）
#[get("/me/vouchers")]
async fn my_vouchers(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<VoucherRow> = sqlx::query_as(
        "SELECT id, kind, source, granted_at, expires_at, used_torrent_id, used_at \
         FROM user_vouchers WHERE user_id = $1 ORDER BY id DESC LIMIT 200",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}
