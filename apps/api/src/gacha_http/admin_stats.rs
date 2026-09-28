//! 后台巡检只读统计：GET /admin/gacha/stats（user.adjust）。
//! 发放端点在 staff.rs；这里只聚合，不重复编辑器（内容进包，方案 §3）。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::authz;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

type Db = web::Data<std::sync::Arc<AppState>>;

#[get("/admin/gacha/stats")]
pub(crate) async fn gacha_admin_stats(
    state: Db,
    req: HttpRequest,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let (banners, cards, draws, lit_users, tickets, shards): (
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
    ) = sqlx::query_as(
        "SELECT \
         (SELECT count(*) FROM gacha_banners WHERE enabled), \
         (SELECT count(*) FROM gacha_cards WHERE enabled), \
         (SELECT count(*) FROM gacha_draws), \
         (SELECT count(DISTINCT user_id) FROM gacha_user_cards), \
         (SELECT COALESCE(SUM(balance), 0) FROM gacha_ticket_balance), \
         (SELECT COALESCE(SUM(balance), 0) FROM gacha_shard_balance)",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "banners": banners,
        "cards": cards,
        "draws": draws,
        "litUsers": lit_users,
        "tickets": tickets,
        "shards": shards,
    })))
}
