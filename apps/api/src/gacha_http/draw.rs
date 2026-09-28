//! G31-B 抽取闭环（方案 §5）：POST /gacha/draw——行锁扣券 → 服务端逐次 roll
//! （seed 落库可复核）→ 写 gacha_draws → 碎片聚合入账 → upsert 持有（lit_at
//! 只首写）→ 返回结果序列与两本余额。
//!
//! 守卫纪律：**运行时**先算含保底综合返还率，>100% 直接 409（PoolGuard）——
//! 后台保存拦截不够，站长直连改库也要被拦。幂等纪律：同 key 重复提交返回
//! 同一批（(idempotency_key, seq) 唯一键兜底 + 事务内先查）；并发走
//! `FOR UPDATE` 行锁，超发/双扣由 CHECK(balance>=0) + 连续 balance_after 兜底。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::gacha_math;
use crate::http::require_auth;
use crate::state::AppState;

use super::{pool, roll};

type Db = web::Data<std::sync::Arc<AppState>>;

#[derive(Deserialize)]
struct DrawReq {
    banner_id: i64,
    count: i32,
    idempotency_key: String,
}

#[post("/gacha/draw")]
pub(super) async fn gacha_draw(
    state: Db,
    req: HttpRequest,
    body: web::Json<DrawReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let user_id = auth.id;
    let count = body.count;
    if !(1..=10).contains(&count) {
        return Err(DomainError::Validation("count 需在 1~10".into()));
    }
    let idem = body.idempotency_key.trim().to_string();
    if idem.len() < 8 || idem.len() > 120 {
        return Err(DomainError::Validation(
            "idempotency_key 需 8~120 字符".into(),
        ));
    }
    let bundle = pool::load_pool(&state.repo.db, body.banner_id).await?;
    // 运行时经济守卫：含保底综合返还率 >100% 直接 409（直改库也拦）
    let ec = gacha_math::economics(&bundle.cfg, bundle.meta.ticket_cost as f64);
    if ec.ratio_worst > 1.0 {
        tracing::error!(
            banner = body.banner_id,
            ratio = ec.ratio_worst,
            sv = ec.sv,
            "gacha pool fails economy guard; draws blocked"
        );
        return Err(DomainError::PoolGuard(format!(
            "综合返还率 {:.1}%",
            ec.ratio_worst * 100.0
        )));
    }
    let cost = bundle.meta.ticket_cost;
    let total = cost * count;
    let ticket_idem = format!("gacha:draw:{idem}");
    let shard_idem = format!("gacha:shard:{idem}");

    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO gacha_ticket_balance (user_id, balance) \
         VALUES ($1, 0) ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let bal: i32 = sqlx::query_scalar(
        "SELECT balance FROM gacha_ticket_balance \
         WHERE user_id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等重放：同 key 已扣过券 → 返回同批结果，不再扣（P2 审计同 tx 口径）
    let replay: Option<i64> = sqlx::query_scalar(
        "SELECT ref_id FROM gacha_ticket_ledger WHERE idempotency_key = $1",
    )
    .bind(&ticket_idem)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(seen_banner) = replay {
        let (prior, shard_balance) =
            pool::prior_draw_rows(&mut tx, user_id, seen_banner, &idem).await?;
        tx.commit()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        return Ok(ok(serde_json::json!({
            "replayed": true, "bannerId": seen_banner,
            "rows": prior, "ticketBalance": bal,
            "shardBalance": shard_balance,
        })));
    }
    if bal < total {
        return Err(DomainError::Validation(format!(
            "抽卡券不足：需要 {total}，持有 {bal}"
        )));
    }
    // pity 状态：最后一批的最后一抽——金档已重置，非金档续计数
    let last: Option<(i32, Option<String>)> = sqlx::query_as(
        "SELECT d.pity_at, d.rarity FROM gacha_draws d \
         WHERE d.user_id = $1 AND d.banner_id = $2 \
         ORDER BY d.id DESC LIMIT 1",
    )
    .bind(user_id)
    .bind(body.banner_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut since = match last {
        Some((p, r)) => match r {
            Some(x) if gacha_math::is_gold(&x) => 0,
            _ => p,
        },
        None => 0,
    };
    let hard = bundle.meta.pity_hard.max(1);
    let seed = roll::batch_seed(user_id);
    let mut results: Vec<serde_json::Value> =
        Vec::with_capacity(count as usize);
    let mut shard_total: i32 = 0;
    for seq in 1..=count {
        since += 1;
        let row_seed = (seed as i64).wrapping_add(seq as i64) as i64;
        let mut rnd = gacha_math::rng(row_seed);
        let pick = roll::roll(&bundle.cfg, since, &mut rnd);
        let row = &bundle.cfg.rates[pick];
        let gold_row = gacha_math::is_gold(&row.r);
        let was_pity = since >= hard;
        let pity_at = roll::pity_at(since, gold_row);
        if gold_row {
            since = 0;
        }
        let mut item = serde_json::json!({
            "seq": seq, "type": row.kind, "r": row.r,
            "shards": row.shards as i32,
            "pityAt": pity_at, "wasPity": was_pity,
        });
        let mut card_id_out: Option<i64> = None;
        let mut is_new = false;
        if row.kind == "shard" {
            shard_total += row.shards as i32;
        } else if row.kind == "card" {
            let card_id = bundle.card_ids[pick].unwrap_or(0);
            if card_id > 0 {
                let lit: Option<chrono::DateTime<chrono::Utc>> =
                    sqlx::query_scalar(
                        "SELECT lit_at FROM gacha_user_cards \
                         WHERE user_id = $1 AND card_id = $2",
                    )
                    .bind(user_id)
                    .bind(card_id)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                is_new = lit.is_none();
                sqlx::query(
                    "INSERT INTO gacha_user_cards \
                     (user_id, card_id, held, lit_at, first_source) \
                     VALUES ($1, $2, 1, now(), $3) \
                     ON CONFLICT (user_id, card_id) \
                     DO UPDATE SET held = gacha_user_cards.held + 1",
                )
                .bind(user_id)
                .bind(card_id)
                .bind(format!("draw:{idem}"))
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                card_id_out = Some(card_id);
                item["isNew"] = serde_json::json!(is_new);
                item["cardId"] = serde_json::json!(card_id);
            }
        }
        sqlx::query(
            "INSERT INTO gacha_draws (user_id, banner_id, seq, \
             output_type, rarity, card_id, shards, pity_at, was_pity, \
             is_new, seed, idempotency_key) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        )
        .bind(user_id)
        .bind(body.banner_id)
        .bind(seq)
        .bind(row.kind.clone())
        .bind(if row.kind == "card" {
            Some(row.r.clone())
        } else {
            None
        })
        .bind(card_id_out)
        .bind(row.shards as i32)
        .bind(pity_at)
        .bind(was_pity)
        .bind(is_new)
        .bind(row_seed)
        .bind(&idem)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        results.push(item);
    }
    // 券流水（扣减一行，balance_after 连续可核）+ 余额更新
    let after = bal - total;
    sqlx::query(
        "INSERT INTO gacha_ticket_ledger (user_id, delta, kind, ref_type, \
         ref_id, idempotency_key, balance_after) \
         VALUES ($1, $2, 'draw', 'banner', $3, $4, $5)",
    )
    .bind(user_id)
    .bind(-total)
    .bind(body.banner_id)
    .bind(&ticket_idem)
    .bind(after)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE gacha_ticket_balance SET balance = $2 WHERE user_id = $1",
    )
    .bind(user_id)
    .bind(after)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 碎片聚合入账（一批一行）
    sqlx::query(
        "INSERT INTO gacha_shard_balance (user_id, balance) \
         VALUES ($1, 0) ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let sbal: i32 = sqlx::query_scalar(
        "SELECT balance FROM gacha_shard_balance \
         WHERE user_id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let s_after = sbal + shard_total;
    sqlx::query(
        "INSERT INTO gacha_shard_ledger (user_id, delta, kind, ref_type, \
         ref_id, idempotency_key, balance_after) \
         VALUES ($1, $2, 'draw', 'banner', $3, $4, $5)",
    )
    .bind(user_id)
    .bind(shard_total)
    .bind(body.banner_id)
    .bind(&shard_idem)
    .bind(s_after)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE gacha_shard_balance SET balance = $2 WHERE user_id = $1",
    )
    .bind(user_id)
    .bind(s_after)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "replayed": false, "bannerId": body.banner_id,
        "rows": results, "ticketBalance": after, "shardBalance": s_after,
    })))
}
