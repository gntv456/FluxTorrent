//! 开放 API：本人魔力概览（Token 鉴权）——插件用户卡的「时魔」来源。
//!
//! 页面口径的 `/me/spark`（economy 域）认登录态 JWT，插件生态手上只有开放
//! API Token（fxo_）→ 插件用户卡的时魔列恒为 0。这里按同一口径开放：
//! 时魔 = 下一个小时的做种收益预估（与 worker 结算同源，见
//! economy_http::seeding_public::bonus_per_hour），不是一个另算的近似值。
//!
//! SQL 在 R 层 / 经济域薄壳，H 层只调函数。

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::repo::user_stats;
use crate::state::AppState;
use actix_web::{get, web, HttpRequest, HttpResponse};

use super::{require_scope, require_token, with_rl, SCOPE_READ};

/// GET /open/me/spark —— 魔力余额 + 时魔 + 时积分。
#[utoipa::path(
    get,
    path = "/api/v1/open/me/spark",
    tag = "我的数据",
    description = "我的魔力概览：balance 余额、hourly_estimate 时魔（每小时收益）、\
hourly_points_estimate 时积分（每小时做种积分）。",
    responses(
        (status = 200,
         description = "balance/hourly_estimate/hourly_points_estimate"),
        (status = 401, description = "Token 无效/已撤销/未传（code 2001）"),
        (status = 403, description = "缺所需 scope（code 2003）"),
        (status = 429, description = "限流（code 1015），带 Retry-After: 60"),
    )
)]
#[get("/open/me/spark")]
pub(super) async fn me_spark(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    require_scope(&tk, SCOPE_READ)?;
    let uid = tk.uid;
    let balance = user_stats::spark_balance(&state.repo, uid).await;
    let hourly_estimate =
        crate::economy_http::bonus_per_hour(&state.repo, uid).await;
    let hourly_points_estimate =
        crate::economy_http::bonus_points_per_hour(&state.repo, uid).await;
    Ok(with_rl(
        ok(serde_json::json!({
            "balance": balance,
            "hourly_estimate": hourly_estimate,
            "hourly_points_estimate": hourly_points_estimate,
        })),
        &tk,
    ))
}
