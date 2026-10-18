//! 牧场 + 加工坊（样图⑤「动物牧场 / 加工坊」子页）。
//! 经济口径：买牲畜/开加工回收魔力；产物经 earn_spark 入账，幂等键
//! 带产出周期的时间戳（同一周期只付一次，防重放白嫖——与浇肥同纪律）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::{earn_spark, spend_spark, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::helpers::{check_rate_scoped, RateScope};

type Db = web::Data<std::sync::Arc<AppState>>;

#[derive(Deserialize)]
pub(super) struct AnimalReq {
    pub(super) animal: String,
    /// 客户端幂等键（可选）：带上来就认它，重复提交只扣一次；不带就是新的一笔。
    /// 牧场买入必须有这个口子——退休会删栏并要求「重新领养」，若把幂等键写死成
    /// `ranch-buy:{uid}:{animal}`，第二次领养会被当成重复提交永久拒（P-334）。
    #[serde(default)]
    pub(super) idempotency_key: Option<String>,
}

#[derive(Deserialize)]
struct RecipeReq {
    recipe: String,
}

/// 牧场 + 加工坊现状（GET /farm/ranch）：目录 × 我的栏位/在产位。
/// 返回的 ready 时间让前端画倒计时；产出在服务端判（防改客户端表）。
#[get("/farm/ranch")]
pub(super) async fn ranch_state(
    req: HttpRequest,
    state: Db,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;
    let animals: Vec<(String, String, String, i64, i64, i32)> = sqlx::query_as(
        "SELECT key, name, icon, price, yield_spark, cycle_mins \
           FROM arcade_ranch_animals WHERE enabled ORDER BY sort",
    )
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let pens: Vec<(String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT animal, ready_at FROM arcade_ranch_pens WHERE user_id = $1",
    )
    .bind(auth.id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let animal_items: Vec<serde_json::Value> = animals
        .iter()
        .map(|(key, name, icon, price, yld, cyc)| {
            let pen = pens.iter().find(|(a, _)| a == key);
            serde_json::json!({
                "key": key, "name": name, "icon": icon,
                "price": price, "yield": yld, "cycle_mins": cyc,
                "owned": pen.is_some(),
                "ready_at": pen.map(|(_, t)| t),
            })
        })
        .collect();
    let recipes: Vec<(String, String, String, i64, i64, i32)> = sqlx::query_as(
        "SELECT key, name, icon, in_spark, out_spark, mins \
           FROM arcade_farm_recipes WHERE enabled ORDER BY sort",
    )
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let crafts: Vec<(String, chrono::DateTime<chrono::Utc>, i64)> =
        sqlx::query_as(
            "SELECT recipe, ready_at, out_spark FROM arcade_farm_crafts \
          WHERE user_id = $1",
        )
        .bind(auth.id)
        .fetch_all(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let recipe_items: Vec<serde_json::Value> = recipes
        .iter()
        .map(|(key, name, icon, spend, yld, mins)| {
            let c = crafts.iter().find(|(r, _, _)| r == key);
            serde_json::json!({
                "key": key, "name": name, "icon": icon,
                "in": spend, "out": yld, "mins": mins,
                "crafting": c.is_some(),
                "ready_at": c.map(|(_, t, _)| t),
            })
        })
        .collect();
    Ok(ok(serde_json::json!({
        "animals": animal_items,
        "recipes": recipe_items,
    })))
}

/// 开工加工（POST /farm/craft）：扣投入 → 落在产位（锁定产出额）。
#[post("/farm/craft")]
pub(super) async fn craft_start(
    req: HttpRequest,
    state: Db,
    body: web::Json<RecipeReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;
    // ZT2（2026-10-03）：加工此前**无任何限次闸**——牧场/加工是全站唯一没有
    // check_rate 的扣魔力互动，配合「产出>投入」可无限刷。补上与农场/宠物同档的限流。
    check_rate_scoped(&state, &state.redis, auth.id, RateScope::Farm).await?;
    let recipe: Option<(i64, i64, i32)> = sqlx::query_as(
        "SELECT in_spark, out_spark, mins FROM arcade_farm_recipes \
          WHERE key = $1 AND enabled",
    )
    .bind(&body.recipe)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((spend, yld, mins)) = recipe else {
        return Err(DomainError::Validation("没有这个配方".into()));
    };
    let busy: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM arcade_farm_crafts \
          WHERE user_id = $1 AND recipe = $2)",
    )
    .bind(auth.id)
    .bind(&body.recipe)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if busy {
        return Err(DomainError::Validation("这个配方已经在做了".into()));
    }
    let idem = format!("craft-start:{}:{}:{}", auth.id, body.recipe, yld);
    match spend_spark(db, auth.id, spend, "game", &idem, "farm_craft", 0)
        .await?
    {
        SpendOutcome::Replayed => {
            return Err(DomainError::Validation("请勿重复提交".into()))
        }
        SpendOutcome::Spent => {}
    }
    let inserted = sqlx::query(
        "INSERT INTO arcade_farm_crafts (user_id, recipe, ready_at, out_spark) \
         VALUES ($1, $2, now() + make_interval(mins => $4::int), $3) \
         ON CONFLICT (user_id, recipe) DO NOTHING",
    )
    .bind(auth.id)
    .bind(&body.recipe)
    .bind(yld)
    .bind(mins)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if inserted.rows_affected() == 0 {
        let _ = earn_spark(
            db,
            auth.id,
            spend,
            "game",
            &format!("craft-start-refund:{}", idem),
        )
        .await;
        return Err(DomainError::Validation("这个配方已经在做了".into()));
    }
    Ok(ok(serde_json::json!({
        "started": body.recipe, "cost": spend, "out": yld,
        "ready_in_mins": mins,
    })))
}

/// 收加工产物（POST /farm/craft/collect）：幂等键带 ready_at。
#[post("/farm/craft/collect")]
pub(super) async fn craft_collect(
    req: HttpRequest,
    state: Db,
    body: web::Json<RecipeReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;
    let row: Option<(chrono::DateTime<chrono::Utc>, i64)> = sqlx::query_as(
        "SELECT ready_at, out_spark FROM arcade_farm_crafts \
          WHERE user_id = $1 AND recipe = $2 AND ready_at <= now() \
          FOR UPDATE",
    )
    .bind(auth.id)
    .bind(&body.recipe)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((ready_at, yld)) = row else {
        return Err(DomainError::Validation("还没做好".into()));
    };
    let deleted = sqlx::query(
        "DELETE FROM arcade_farm_crafts \
          WHERE user_id = $1 AND recipe = $2 AND ready_at = $3",
    )
    .bind(auth.id)
    .bind(&body.recipe)
    .bind(ready_at)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if deleted.rows_affected() == 0 {
        return Err(DomainError::Validation("手慢了，已被收走".into()));
    }
    earn_spark(
        db,
        auth.id,
        yld,
        "arcade",
        &format!("craft-collect:{}:{}:{}", auth.id, body.recipe, ready_at),
    )
    .await?;
    Ok(ok(
        serde_json::json!({ "collected": body.recipe, "earned": yld }),
    ))
}
