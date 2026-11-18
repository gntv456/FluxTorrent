//! 我的邀请概览口（从 invite_http.rs 拆出——B 方案探测闸把它顶过 300 门禁）。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::invite_quota_limit;
use super::quota;

#[get("/invites/status")]
pub async fn invites_status_handler(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let limit = invite_quota_limit(&state, &auth).await;
    let (used, extra): (i32, i32) = sqlx::query_as(&format!(
        "SELECT \
            COALESCE((SELECT used FROM invite_quota \
                      WHERE user_id = $1 AND period = {}), 0), \
            COALESCE(quota_extra, 0) \
         FROM users WHERE id = $1",
        quota::WEEK_PERIOD,
    ))
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or((0, 0));
    let price: Option<i64> = sqlx::query_scalar(
        "SELECT price FROM shop_items WHERE kind = 'invite' AND active \
         = true ORDER BY price LIMIT 1",
    )
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 三个计数与列表筛选同一口径：已过期同时认存储态 status=2
    //（历史码/管理作废）与「未用且已过期」，否则角标数 ≠ 筛选结果数。
    let inv_stats: (i64, i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE status = 0 AND expires_at > now()), \
                count(*) FILTER (WHERE status = 1), \
                count(*) FILTER (WHERE status = 2 \
                    OR (status = 0 AND expires_at <= now())) \
         FROM invites WHERE inviter_id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (unused, used_count, expired) = inv_stats;
    // B 方案（2026-11-18）：SMTP 可达性随 status 下发——前端据此在发信弹层
    // 提前警示/置灰，而不是点了才等 4s 探测。探测有成本（建连+NOOP），
    // 60s 内存缓存防 status 轮询把 SMTP 打成探针。
    static SMTP_OK: tokio::sync::RwLock<Option<(bool, std::time::Instant)>> =
        tokio::sync::RwLock::const_new(None);
    let smtp_ready = {
        let cached = *SMTP_OK.read().await;
        match cached {
            Some((v, at)) if at.elapsed().as_secs() < 60 => v,
            _ => {
                let v = match crate::mailer::smtp_config(&state.repo.db).await {
                    Some(c) => {
                        crate::mailer::probe::smtp_reachable(&c.url).await
                    }
                    None => false,
                };
                *SMTP_OK.write().await = Some((v, std::time::Instant::now()));
                v
            }
        }
    };
    Ok(ok(serde_json::json!({
        "class_id": auth.class_id,
        "quota_limit": limit,
        "quota_used": used,
        "quota_extra": extra,
        "redeem_price": price,
        "unused": unused, "used": used_count, "expired": expired,
        "smtp_ready": smtp_ready,
    })))
}
