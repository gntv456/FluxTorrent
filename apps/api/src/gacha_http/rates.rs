//! G31-A 只读公示端点：稀有度 token / 卡定义 / 卡池列表 / 单池概率双列。
//! 全部匿名可读、只读、无经济行为；概率与经济口径出自 `crate::gacha_math`
//! 单源；池重建走 `super::pool::load_pool`（与 draw 同一份，不许各写一份）。

use actix_web::{get, web, HttpResponse};
use serde::Serialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::gacha_math;
use crate::state::AppState;

use super::pool;

type Db = web::Data<std::sync::Arc<AppState>>;

#[derive(Serialize)]
struct RateOut {
    r: String,
    #[serde(rename = "type")]
    kind: String,
    base: f64,
    comp: f64,
    value: f64,
    shards: i32,
    dupe: f64,
    synth: f64,
    lv_max: f64,
    lv_step: f64,
    lv_gain: f64,
}

#[get("/gacha/rarities")]
pub(super) async fn gacha_rarities(state: Db) -> DomainResult<HttpResponse> {
    let rows: Vec<(
        String,
        String,
        i32,
        i16,
        i16,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        f64,
        String,
    )> = sqlx::query_as(
        "SELECT key, label, sort, stars, gold_rank, frame, frame_hi, \
         glow, bg1, bg2, ink, foil::float8, foil_mask \
         FROM gacha_rarities WHERE enabled ORDER BY sort",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let body = serde_json::json!(rows
        .iter()
        .map(
            |(
                key,
                label,
                sort,
                stars,
                gold,
                frame,
                hi,
                glow,
                bg1,
                bg2,
                ink,
                foil,
                mask,
            )| {
                serde_json::json!({
                    "key": key, "label": label, "sort": sort,
                    "stars": stars, "goldRank": gold, "frame": frame,
                    "frameHi": hi, "glow": glow, "bg1": bg1, "bg2": bg2,
                    "ink": ink, "foil": foil, "foilMask": mask,
                })
            },
        )
        .collect::<Vec<_>>());
    Ok(ok(body))
}

#[get("/gacha/cards")]
pub(super) async fn gacha_cards(state: Db) -> DomainResult<HttpResponse> {
    let rows: Vec<(
        i64,
        String,
        String,
        String,
        Option<String>,
        Option<i64>,
        Option<String>,
        Option<String>,
        i32,
        i32,
        i16,
        i32,
        f64,
    )> = sqlx::query_as(
        "SELECT id, key, name, rarity, series_kind, series_value, \
         entity_table, entity_ref, synth_shards, dupe_shards, lv_max, \
         lv_cost_each, lv_gain::float8 \
         FROM gacha_cards WHERE enabled ORDER BY sort",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let body = serde_json::json!(rows
        .iter()
        .map(
            |(
                id,
                key,
                name,
                rarity,
                sk,
                sv,
                et,
                er,
                synth,
                dupe,
                lv_max,
                lv_step,
                lv_gain,
            )| {
                serde_json::json!({
                    "id": id, "key": key, "name": name, "rarity": rarity,
                    "seriesKind": sk, "seriesValue": sv,
                    "entityTable": et, "entityRef": er,
                    "synthShards": synth, "dupeShards": dupe,
                    "lvMax": lv_max, "lvCostEach": lv_step,
                    "lvGain": lv_gain,
                })
            },
        )
        .collect::<Vec<_>>());
    Ok(ok(body))
}

#[get("/gacha/banners")]
pub(super) async fn gacha_banners(state: Db) -> DomainResult<HttpResponse> {
    let rows: Vec<(i64, String, String, String, i32)> = sqlx::query_as(
        "SELECT id, key, name, kind, ticket_cost \
         FROM gacha_banners WHERE enabled \
         AND (starts_at IS NULL OR starts_at <= now()) \
         AND (ends_at IS NULL OR ends_at > now()) \
         ORDER BY id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let body = serde_json::json!(rows
        .iter()
        .map(|(id, key, name, kind, cost)| {
            serde_json::json!({
                "id": id, "key": key, "name": name, "kind": kind,
                "ticketCost": cost,
            })
        })
        .collect::<Vec<_>>());
    Ok(ok(body))
}

/// 单池概率公示：**表定 base 与含保底综合 comp 双列**（方案红线：只公示
/// 表定会放行实际倒灌经济的池子）。数学输出直接来自 gacha_math 单源。
#[get("/gacha/banner/{id}/rates")]
pub(super) async fn gacha_banner_rates(
    state: Db,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let id = path.into_inner();
    let bundle = pool::load_pool(&state.repo.db, id).await?;
    let co = gacha_math::composite(&bundle.cfg);
    let ec = gacha_math::economics(&bundle.cfg, bundle.meta.ticket_cost as f64);
    let comp_sum: f64 = co.comp.iter().sum();
    let rows: Vec<RateOut> = co
        .base
        .iter()
        .enumerate()
        .map(|(i, base)| {
            let r = &bundle.cfg.rates[i];
            RateOut {
                r: r.r.clone(),
                kind: r.kind.clone(),
                base: *base,
                comp: co.comp[i],
                value: r.value,
                shards: r.shards as i32,
                dupe: r.dupe,
                synth: r.synth,
                lv_max: r.lv_max,
                lv_step: r.lv_step,
                lv_gain: r.lv_gain,
            }
        })
        .collect();
    let m = &bundle.meta;
    let body = serde_json::json!({
        "banner": {
            "id": m.id, "key": m.key, "kind": m.kind,
            "ticketCost": m.ticket_cost, "pitySoft": m.pity_soft,
            "pityHard": m.pity_hard, "pityRamp": m.pity_ramp,
        },
        "rows": rows,
        "goldBase": co.gold_base,
        "goldComp": co.gold_comp,
        "cycle": co.cycle,
        "hardProb": co.hard_prob,
        "evComp": co.ev_comp,
        "degenerate": co.degenerate,
        "sv": ec.sv,
        "svFrom": ec.sv_from.as_ref().map(|s| {
            serde_json::json!({ "kind": s.kind, "r": s.r })
        }),
        "evNew": ec.ev_new,
        "evEnd": ec.ev_end,
        "ratioNew": ec.ratio_new,
        "ratioEnd": ec.ratio_end,
        "ratioWorst": ec.ratio_worst,
        "compSum": comp_sum,
    });
    Ok(ok(body))
}
