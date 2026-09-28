//! 奖池配置与抽取行读取单源：banner 元数据 + 奖池行 + 卡片 card_id 平行向量
//! + 幂等重放行。rates 公示与 draw 抽取共用同一份重建（对齐
//! `gacha_math_vectors.json` 的 RateRow 口径）——两处各写一份必然漂移
//! （方案 §3「四处同源」纪律）。
//!
//! 注意（0228 真实列名）：prize_value 在行上，卡列只有 synth/dupe/lv_*；
//! `gacha_banners` 无 is_on（缺省开在 default_on 白名单 + 值行）。

use crate::errors::{DomainError, DomainResult};
use crate::gacha_math;

/// banner 元数据（draw 扣券 / 公示头都从这里拿）。
pub(super) struct BannerMeta {
    pub id: i64,
    pub key: String,
    pub kind: String,
    pub ticket_cost: i32,
    pub pity_soft: i32,
    pub pity_hard: i32,
    pub pity_ramp: f64,
}

pub(super) struct PoolBundle {
    pub cfg: gacha_math::PoolConfig,
    /// 与 cfg.rates 同序：卡档的 gacha_cards.id（miss/shard 档为 None）。
    pub card_ids: Vec<Option<i64>>,
    pub meta: BannerMeta,
}

pub(super) async fn load_pool(
    db: &sqlx::PgPool,
    banner_id: i64,
) -> DomainResult<PoolBundle> {
    let banner: Option<(String, String, i32, i32, i32, f64)> = sqlx::query_as(
        "SELECT key, kind, ticket_cost, pity_soft, pity_hard, \
         pity_ramp::float8 FROM gacha_banners WHERE id = $1 AND enabled",
    )
    .bind(banner_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((key, kind, cost, soft, hard, ramp)) = banner else {
        return Err(DomainError::NotFound(banner_id));
    };
    let rows: Vec<(
        String,
        String,
        f64,
        f64,
        i32,
        Option<i64>,
        i32,
        i32,
        i32,
        i32,
        f64,
    )> = sqlx::query_as(
        "SELECT r.output_type, COALESCE(r.rarity, ''), r.weight::float8, \
         r.prize_value::float8, r.shards, r.card_id, \
         COALESCE(c.synth_shards, 0), COALESCE(c.dupe_shards, 0), \
         COALESCE(c.lv_max, 1)::int, COALESCE(c.lv_cost_each, 0), \
         COALESCE(c.lv_gain::float8, 0) \
         FROM gacha_pool_rows r \
         LEFT JOIN gacha_cards c ON c.id = r.card_id \
         WHERE r.banner_id = $1 ORDER BY r.sort",
    )
    .bind(banner_id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut card_ids: Vec<Option<i64>> = Vec::with_capacity(rows.len());
    let rates: Vec<gacha_math::RateRow> = rows
        .iter()
        .map(
            |(
                ty,
                rar,
                w,
                prize,
                shards,
                card_id,
                synth,
                dupe,
                lv_max,
                lv_step,
                lv_gain,
            )| {
                card_ids.push(*card_id);
                gacha_math::RateRow {
                    r: if rar.is_empty() {
                        if ty == "shard" { "SHARD" } else { "MISS" }.into()
                    } else {
                        rar.clone()
                    },
                    kind: ty.clone(),
                    w: *w,
                    value: if ty == "card" { *prize } else { 0.0 },
                    shards: *shards as f64,
                    dupe: *dupe as f64,
                    synth: *synth as f64,
                    lv_max: *lv_max as f64,
                    lv_step: *lv_step as f64,
                    lv_gain: *lv_gain,
                }
            },
        )
        .collect();
    let cfg = gacha_math::PoolConfig {
        rates,
        pity: gacha_math::PityConfig {
            soft: soft as f64,
            hard: hard as f64,
            ramp,
            up_ratio: None,
            guarantee_up: false,
        },
    };
    Ok(PoolBundle {
        cfg,
        card_ids,
        meta: BannerMeta {
            id: banner_id,
            key,
            kind,
            ticket_cost: cost,
            pity_soft: soft,
            pity_hard: hard,
            pity_ramp: ramp,
        },
    })
}

/// 幂等重放取回同批抽取行 + 当前碎片余额（pity_at 口径与首写一致）。
pub(super) async fn prior_draw_rows(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    banner_id: i64,
    idem: &str,
) -> DomainResult<(serde_json::Value, i32)> {
    let rows: Vec<(
        i32,
        String,
        Option<String>,
        Option<i64>,
        i32,
        i32,
        bool,
        bool,
    )> = sqlx::query_as(
        "SELECT seq, output_type, rarity, card_id, shards, pity_at, \
         was_pity, is_new FROM gacha_draws \
         WHERE user_id = $1 AND banner_id = $2 AND idempotency_key = $3 \
         ORDER BY seq",
    )
    .bind(user_id)
    .bind(banner_id)
    .bind(idem)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let shard_balance: i32 = sqlx::query_scalar(
        "SELECT COALESCE(balance, 0) FROM gacha_shard_balance \
         WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or(0);
    let rows = serde_json::json!(rows
        .iter()
        .map(|(seq, ty, rar, card, shards, pity, wp, is_new)| {
            serde_json::json!({
                "seq": seq, "type": ty, "r": rar, "cardId": card,
                "shards": shards, "pityAt": pity, "wasPity": wp,
                "isNew": is_new,
            })
        })
        .collect::<Vec<_>>());
    Ok((rows, shard_balance))
}
