//! 农场·图鉴（样图⑤「图鉴」子页）：作物图鉴 = farm_crops 全集 ×
//! 用户是否种过/收过；动物图鉴 = 牧场种属 × 是否养过（牧场批落地后
//! 同一张表喂牲畜）。读侧聚合，不给写路径。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

type Db = web::Data<std::sync::Arc<AppState>>;

#[get("/farm/collection")]
pub(super) async fn farm_collection(
    req: HttpRequest,
    state: Db,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;

    // 作物：全集（含下架）× 用户种过的作物 id 集（种过 = 点亮）
    let crops: Vec<(i32, String, bool)> = sqlx::query_as(
        "SELECT c.id, c.name, c.active FROM farm_crops c ORDER BY c.id",
    )
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let planted: Vec<(i32,)> = sqlx::query_as(
        "SELECT DISTINCT crop_id FROM farm_plots WHERE user_id = $1 \
          UNION SELECT DISTINCT crop_id FROM farm_harvests WHERE user_id = $1",
    )
    .bind(auth.id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let planted_ids: Vec<i32> = planted.into_iter().map(|r| r.0).collect();

    let crop_items: Vec<serde_json::Value> = crops
        .iter()
        .map(|(id, name, active)| {
            let lit = planted_ids.contains(id);
            serde_json::json!({
                "id": id, "name": name, "active": active, "lit": lit,
            })
        })
        .collect();
    let crop_total = crop_items.len() as i64;
    let crop_got = crop_items
        .iter()
        .filter(|c| c["lit"].as_bool().unwrap_or(false))
        .count() as i64;

    // 动物：牧场批未上车站内没有牲畜数据源——如实返回空集，前端整段隐藏
    let animal_items: Vec<serde_json::Value> = Vec::new();

    Ok(ok(serde_json::json!({
        "crops": { "got": crop_got, "total": crop_total, "items": crop_items },
        "animals": { "got": 0, "total": 0, "items": animal_items },
    })))
}
