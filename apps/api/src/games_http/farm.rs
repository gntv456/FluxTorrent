//! 农场（M24）：作物/种植/浇水/收获。
//! 从 games_http.rs 按域拆出；限次助手在 helpers.rs。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::games;
use crate::http::require_auth;
use crate::state::AppState;

use super::helpers::{eco_i64, farm_wither_days, limit_used};

pub(super) async fn get_crop(
    db: &sqlx::PgPool,
    crop_id: i32,
) -> DomainResult<Option<CropRow>> {
    sqlx::query_as(
        "SELECT id, name, seed_price, base_yield, grow_hours, \
         0::bigint AS market_price FROM farm_crops WHERE id = $1",
    )
    .bind(crop_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

/// 农场总览：作物行情（含当前窗口市场价）+ 我的 6 块地
#[get("/farm")]
pub(super) async fn farm_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let now = chrono::Utc::now().timestamp();
    let window = games::market_window_start(now);

    let crops: Vec<CropRow> = sqlx::query_as(
        "SELECT id, name, seed_price, base_yield, grow_hours, \
         0::bigint AS market_price FROM farm_crops ORDER BY id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let crops: Vec<_> = crops
        .into_iter()
        .map(|mut c| {
            c.market_price = games::market_price(c.seed_price as i64, window);
            c
        })
        .collect();

    let wither_days = farm_wither_days(&state).await;
    let plots: Vec<PlotRow> = sqlx::query_as(
        r#"SELECT p.slot, p.crop_id, c.name AS crop_name, p.planted_at, p.ready_at, p.watered,
            (p.ready_at <= now()) AS ready,
            ($2 > 0 AND p.ready_at + make_interval(days => $2::int) < now()) AS withered
         FROM farm_plots p JOIN farm_crops c ON c.id = p.crop_id
         WHERE p.user_id = $1 ORDER BY p.slot"#,
    )
    .bind(auth.id)
    .bind(wither_days)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 农场自己的限流配额（rl:farm，与即时赌局分开计数）——前端显示「今日可操作 N 次」
    let farm_limit = eco_i64(&state, "farm_max_plays_per_hour", 30).await;
    let farm_left = (farm_limit
        - limit_used(&state, auth.id, true).await.unwrap_or(0))
    .max(0);

    Ok(ok(serde_json::json!({
        "window_start": window,
        "next_refresh": window + 4 * 3600,
        "crops": crops,
        "plots": plots,
        "slots": 6,
        "wither_days": wither_days,
        "hour_limit": farm_limit,
        "hour_left": farm_left,
    })))
}

#[derive(Deserialize)]
pub(super) struct PlantReq {
    pub(super) slot: i32,
    pub(super) crop_id: i32,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub(super) struct CropRow {
    pub(super) id: i32,
    pub(super) name: String,
    pub(super) seed_price: i32,
    pub(super) base_yield: i32,
    pub(super) grow_hours: i32,
    pub(super) market_price: i64,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub(super) struct PlotRow {
    pub(super) slot: i32,
    pub(super) crop_id: i32,
    pub(super) crop_name: String,
    pub(super) planted_at: chrono::DateTime<chrono::Utc>,
    pub(super) ready_at: chrono::DateTime<chrono::Utc>,
    pub(super) watered: bool,
    pub(super) ready: bool,
    /// 成熟后超过 `farm_wither_days` 天未收获 → 枯萎（收获作废、清空地块）
    pub(super) withered: bool,
}
