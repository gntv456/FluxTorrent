//! 宠物自定义（改名 / 选种）与个人页展示位。
//!
//! 种类只改外观（emoji 成长线由前端按 species 渲染），**不改任何数值** ——
//! 数值差异化就是 pay-to-win 的第一块砖。展示位走只读快照：个人页是高频
//! 读，按 `last_tick` 现算展示值但不落写（落写交给宠物页的 tick）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::helpers::eco_i64;

/// 在册种类（与前端 `pet-pen.tsx` 的成长线一一对应；加种类两端同步）
const SPECIES: [&str; 4] = ["slime", "cat", "bunny", "drake"];

#[derive(Deserialize)]
pub(super) struct CustomizeReq {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    species: Option<String>,
}

/// 改名 / 选种：都不填就是一次空操作（返回现状），填了才改。
/// 天然幂等（同参数重复提交结果一致），不需要幂等键。
#[post("/games/pet/customize")]
pub(super) async fn pet_customize(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: Option<web::Json<CustomizeReq>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let b = body.map(|j| j.into_inner()).unwrap_or(CustomizeReq {
        name: None,
        species: None,
    });
    sqlx::query(
        "INSERT INTO arcade_pets (user_id) VALUES ($1) \
         ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(super::pool::dberr)?;
    let cur: (String, String) = sqlx::query_as(
        "SELECT species, name FROM arcade_pets WHERE user_id = $1",
    )
    .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(super::pool::dberr)?;
    let name = match b.name.as_deref().map(str::trim) {
        Some(n) => {
            if n.chars().count() > 12 {
                return Err(DomainError::Validation("名字最长 12 个字".into()));
            }
            n.to_string()
        }
        None => cur.1,
    };
    let species = match b.species.as_deref() {
        Some(s) if SPECIES.contains(&s) => s.to_string(),
        Some(s) => {
            return Err(DomainError::Validation(format!(
                "没有「{s}」这种宠物：可选 {}",
                SPECIES.join("/")
            )))
        }
        None => cur.0,
    };
    sqlx::query(
        "UPDATE arcade_pets SET name = $2, species = $3 WHERE user_id = $1",
    )
    .bind(auth.id)
    .bind(&name)
    .bind(&species)
    .execute(&state.repo.db)
    .await
    .map_err(super::pool::dberr)?;
    state
        .repo
        .audit(Some(auth.id), "game.pet.customize", None)
        .await;
    Ok(ok(serde_json::json!({
        "name": name, "species": species,
    })))
}

#[derive(Deserialize)]
pub(super) struct PetOfQuery {
    user_id: i64,
}

/// 个人页展示位：只读快照（现算饥饿/能量，不写行、不领取）。
#[get("/games/pet/of")]
pub(super) async fn pet_of(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<PetOfQuery>,
) -> DomainResult<HttpResponse> {
    require_auth(&req, &state).await?;
    let row: Option<(String, String, i32, i64, i32, i64, i64, DateTime<Utc>)> =
        sqlx::query_as(
            "SELECT species, name, level, exp, hunger, energy, pending, \
                    last_tick \
               FROM arcade_pets WHERE user_id = $1",
        )
        .bind(q.user_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(super::pool::dberr)?;
    let Some((species, name, level, exp, hunger, energy, pending, last)) = row
    else {
        return Ok(ok(serde_json::json!({ "exists": false })));
    };
    // 与 pets.rs::tick 同式的只读镜像：时间推进算给前端看，不落库
    let digest = eco_i64(&state, "pet_digest_per_hour", 300)
        .await
        .clamp(1, 100_000);
    let dt_ms = (Utc::now() - last).num_milliseconds().max(0);
    let digested = ((dt_ms * digest) / 3_600_000).min(energy);
    let hunger = (i64::from(hunger) - (dt_ms * 8) / 3_600_000).max(0);
    Ok(ok(serde_json::json!({
        "exists": true,
        "species": species, "name": name,
        "level": level, "exp": exp,
        "hunger": hunger, "energy": energy - digested,
        "pending": pending,
    })))
}
