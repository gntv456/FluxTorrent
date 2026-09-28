//! G31-C 碎片消费三动作（方案 §5）：分解重复卡 / 定向兑换 / 升级。
//!
//! 共同纪律：
//! - 碎片账本行锁 + 幂等键 + balance_after 连续（同 spark/gacha_ticket 形状）；
//! - 分解**只减 held 不动 lit_at**（「图鉴只增不减」落在列上，0232 注释）；
//! - 兑换限次走 `gacha_exchanges` 唯一键（并发双兑数据库兜底，冲突 → 409）；
//! - 升级消耗 `lv_cost_each × 当前等级`（方案 §1.5），满级拒 400；
//! - 升级**只改卡面与图鉴权重**，不给任何站点经济加成——这是红线，加成类
//!   字段不许出现在任何返回或升级路径里。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

type Db = web::Data<std::sync::Arc<AppState>>;

#[derive(Deserialize)]
struct CardReq {
    card_id: i64,
    #[serde(default)]
    idempotency_key: String,
}

fn check_idem(idem: &str) -> DomainResult<()> {
    let t = idem.trim();
    if t.len() < 8 || t.len() > 120 {
        return Err(DomainError::Validation(
            "idempotency_key 需 8~120 字符".into(),
        ));
    }
    Ok(())
}

/// 碎片行锁 + 流水一行 + 余额更新（负 delta 由调用方校验充足）。
async fn shard_move(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    delta: i32,
    kind: &str,
    ref_type: &str,
    ref_id: i64,
    idem: &str,
) -> DomainResult<i32> {
    let bal: i32 = sqlx::query_scalar(
        "SELECT balance FROM gacha_shard_balance \
         WHERE user_id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let after = bal + delta;
    if after < 0 {
        return Err(DomainError::Validation(format!(
            "碎片不足：需要 {}，持有 {bal}",
            -delta
        )));
    }
    sqlx::query(
        "INSERT INTO gacha_shard_ledger (user_id, delta, kind, ref_type, \
         ref_id, idempotency_key, balance_after) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(user_id)
    .bind(delta)
    .bind(kind)
    .bind(ref_type)
    .bind(ref_id)
    .bind(idem)
    .bind(after)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE gacha_shard_balance SET balance = $2 WHERE user_id = $1",
    )
    .bind(user_id)
    .bind(after)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(after)
}

/// POST /gacha/dismantle：分解一张重复卡（held-1，lit_at 不动），按
/// dupe_shards 得碎片。held=1（最后一张）拒 400——图鉴点亮的那张不许化。
#[post("/gacha/dismantle")]
pub(super) async fn gacha_dismantle(
    state: Db,
    req: HttpRequest,
    body: web::Json<CardReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;
    let idem = format!("gacha:dis:{}", body.idempotency_key.trim());
    check_idem(&idem)?;
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等：同 key 已入账 → 拒重复（分解不支持重放语义，防连点双化）
    let seen: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM gacha_shard_ledger \
         WHERE idempotency_key = $1)",
    )
    .bind(&idem)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if seen {
        return Err(DomainError::Validation("重复的分解请求".into()));
    }
    let row: Option<(i32, i32)> = sqlx::query_as(
        "SELECT held, dupe_shards FROM gacha_user_cards uc \
         JOIN gacha_cards c ON c.id = uc.card_id \
         WHERE uc.user_id = $1 AND uc.card_id = $2 FOR UPDATE OF uc",
    )
    .bind(uid)
    .bind(body.card_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((held, dupe)) = row else {
        return Err(DomainError::NotFound(body.card_id));
    };
    if held <= 1 {
        return Err(DomainError::Validation(
            "最后一张卡不可分解（图鉴点亮卡）".into(),
        ));
    }
    if dupe <= 0 {
        return Err(DomainError::Validation("该卡未配置分解值".into()));
    }
    sqlx::query(
        "UPDATE gacha_user_cards SET held = held - 1 \
         WHERE user_id = $1 AND card_id = $2",
    )
    .bind(uid)
    .bind(body.card_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let after = shard_move(
        &mut tx,
        uid,
        dupe,
        "dismantle",
        "card",
        body.card_id,
        &idem,
    )
    .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "cardId": body.card_id, "shardsGained": dupe, "shardBalance": after,
    })))
}

/// POST /gacha/exchange：碎片定向兑卡（每卡每 season 一次，唯一键兜底）。
#[post("/gacha/exchange")]
pub(super) async fn gacha_exchange(
    state: Db,
    req: HttpRequest,
    body: web::Json<CardReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;
    let idem = format!("gacha:exc:{}", body.idempotency_key.trim());
    check_idem(&idem)?;
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let card: Option<(i32, i32)> = sqlx::query_as(
        "SELECT synth_shards, 1 FROM gacha_cards c \
         WHERE c.id = $1 AND c.enabled",
    )
    .bind(body.card_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((synth, _)) = card else {
        return Err(DomainError::NotFound(body.card_id));
    };
    if synth <= 0 {
        return Err(DomainError::Validation("该卡未开放兑换".into()));
    }
    let after =
        shard_move(&mut tx, uid, -synth, "synth", "card", body.card_id, &idem)
            .await?;
    // 限次：唯一键冲突 → 409（shard_move 已在同一事务，冲突整体回滚）
    let inserted = sqlx::query(
        "INSERT INTO gacha_exchanges (user_id, card_id, season_key, \
         shards_spent) VALUES ($1, $2, 'all', $3) ON CONFLICT DO NOTHING",
    )
    .bind(uid)
    .bind(body.card_id)
    .bind(synth)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if inserted.rows_affected() == 0 {
        return Err(DomainError::LedgerConflict);
    }
    // 兑换即持有 +1（lit_at 首写）
    sqlx::query(
        "INSERT INTO gacha_user_cards (user_id, card_id, held, lit_at, \
         first_source) VALUES ($1, $2, 1, now(), $3) \
         ON CONFLICT (user_id, card_id) \
         DO UPDATE SET held = gacha_user_cards.held + 1",
    )
    .bind(uid)
    .bind(body.card_id)
    .bind("exchange")
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "cardId": body.card_id, "shardsSpent": synth, "shardBalance": after,
    })))
}

/// POST /gacha/levelup：升级（消耗 lv_cost_each × 当前等级；满级拒 400）。
#[post("/gacha/levelup")]
pub(super) async fn gacha_levelup(
    state: Db,
    req: HttpRequest,
    body: web::Json<CardReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;
    let idem = format!("gacha:lv:{}", body.idempotency_key.trim());
    check_idem(&idem)?;
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let row: Option<(i32, i32, i32)> = sqlx::query_as(
        "SELECT c.lv_max::int, uc.lv::int, c.lv_cost_each \
         FROM gacha_user_cards uc JOIN gacha_cards c ON c.id = uc.card_id \
         WHERE uc.user_id = $1 AND uc.card_id = $2 FOR UPDATE OF uc",
    )
    .bind(uid)
    .bind(body.card_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((lv_max, lv, cost_each)) = row else {
        return Err(DomainError::NotFound(body.card_id));
    };
    if lv >= lv_max {
        return Err(DomainError::Validation("已满级".into()));
    }
    let cost = cost_each * lv as i32;
    let after =
        shard_move(&mut tx, uid, -cost, "levelup", "card", body.card_id, &idem)
            .await?;
    sqlx::query(
        "UPDATE gacha_user_cards SET lv = lv + 1 \
         WHERE user_id = $1 AND card_id = $2",
    )
    .bind(uid)
    .bind(body.card_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "cardId": body.card_id, "lv": lv + 1, "lvMax": lv_max,
        "shardsSpent": cost, "shardBalance": after,
    })))
}
