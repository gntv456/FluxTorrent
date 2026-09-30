//! 农场（M24）：作物/种植/浇水/收获。
//! 从 games_http.rs 按域拆出；限次助手在 helpers.rs。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::games;
use crate::http::require_auth;
use crate::state::AppState;

use super::helpers::{
    eco_i64, farm_market_hours, farm_wither_days, limit_used,
};

pub(super) async fn get_crop(
    db: &sqlx::PgPool,
    crop_id: i32,
) -> DomainResult<Option<CropRow>> {
    sqlx::query_as(
        "SELECT id, name, seed_price, base_yield, grow_hours, active, \
         0::bigint AS market_price FROM farm_crops WHERE id = $1",
    )
    .bind(crop_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

/// 取一株**可播种**的作物：不存在与已下架分开报 —— 前者是数据坏了，
/// 后者是站长刚把它下架，玩家该看到的是第二句（已种下的地仍然可收）。
pub(super) async fn get_plantable_crop(
    db: &sqlx::PgPool,
    crop_id: i32,
) -> DomainResult<CropRow> {
    let crop = get_crop(db, crop_id)
        .await?
        .ok_or_else(|| DomainError::Validation("作物不存在".into()))?;
    if !crop.active {
        return Err(DomainError::Validation(format!(
            "「{}」已下架，买不到种子（已种下的地还能收）",
            crop.name
        )));
    }
    Ok(crop)
}

/// 农场总览：作物行情（含当前窗口市场价）+ 我的 6 块地
#[get("/farm")]
pub(super) async fn farm_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let now = chrono::Utc::now().timestamp();
    let hours = farm_market_hours(&state).await;
    let window = games::market_window_start_with(now, hours);

    let crops: Vec<CropRow> = sqlx::query_as(
        "SELECT id, name, seed_price, base_yield, grow_hours, active, \
         0::bigint AS market_price FROM farm_crops WHERE active ORDER BY id",
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

    // 土地阶梯状态（买地/升级的价格与等级）。参数配错时这里就报出去 ——
    // 白送地块是另一种增发，不能让它带着坏配置继续长作物。
    let land = super::farm_land::land_projection(&state, auth.id).await?;

    Ok(ok(serde_json::json!({
        "window_start": window,
        // 刷新点由**设置键**决定，不是写死的 4 小时：站长把窗口改成 6，
        // 这里跟着走，否则前端倒计时会指向一个价格不会变的时刻
        "next_refresh": window + hours * 3600,
        "market_refresh": super::helpers::market_refresh_text(hours),
        "crops": crops,
        "plots": plots,
        "slots": games::FARM_PLOTS,
        "land": land,
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
    /// 下架（false）的作物不进行情、不能播种；但**已经种下的地块照常可收**，
    /// 所以地块那两处 JOIN 不过滤它。
    pub(super) active: bool,
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
