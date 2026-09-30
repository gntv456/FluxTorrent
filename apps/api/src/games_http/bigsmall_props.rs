//! 猜大小的「道具加权」（2026-09-30）。
//!
//! 口径：**道具只加权魔力的输赢，永不出物品** —— 猜大小仍是「只输赢魔力」。
//!   · mult   —— 倍率券：赢时派彩 ×value/1000；
//!   · shield —— 护盾：输时返还 value/1000 注额（只减少亏损，不改输赢概率）。
//!
//! 道具不是奖品：它是 `arcade_items` 里 `use_kind='game'` 的一类目录项，
//! 持有数由发放账 − 消耗账反推（`arcade_item_held`），与背包同一份口径。

use serde_json::Value;
use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

use super::pool::dberr;

/// 倍率券的合法区间（千分）：×1 ~ ×5。上限是 EV 闸外的第二道闸 ——
/// 道具越权放大派彩，站点回收口会被单局击穿。
const MULT_MIN: i64 = 1000;
const MULT_MAX: i64 = 5000;
/// 护盾的合法区间（千分）：0.1% ~ 100%
const SHIELD_MAX: i64 = 1000;

#[derive(Clone)]
pub(super) struct Prop {
    pub key: String,
    pub name: String,
    pub icon: String,
    /// mult | shield
    pub effect: String,
    /// 千分值（见文件头）
    pub value: i64,
}

/// 校验一件道具的 `game_effect` 形状（写侧与读侧共用）。
/// 形状不对 = 挂载时不知道改什么，必须当场拦下，别等到玩家点了按钮才炸。
pub(super) fn check_effect_shape(
    v: &Option<Value>,
    name: &str,
) -> DomainResult<()> {
    let Some(v) = v else {
        return Err(DomainError::Validation(format!(
            "「{name}」是游戏道具却没有 game_effect：挂载时无从知道改什么"
        )));
    };
    let game = v.get("game").and_then(|x| x.as_str()).unwrap_or("");
    let effect = v.get("effect").and_then(|x| x.as_str()).unwrap_or("");
    let value = v.get("value").and_then(|x| x.as_i64()).unwrap_or(0);
    if game.is_empty() {
        return Err(DomainError::Validation(format!(
            "「{name}」的 game_effect.game 不能为空"
        )));
    }
    match effect {
        "mult" if !(MULT_MIN..=MULT_MAX).contains(&value) => {
            Err(DomainError::Validation(format!(
                "「{name}」倍率 value={value}‰ 越界（须 {MULT_MIN}~{MULT_MAX}）"
            )))
        }
        "shield" if !(1..=SHIELD_MAX).contains(&value) => {
            Err(DomainError::Validation(format!(
                "「{name}」护盾 value={value}‰ 越界（须 1~{SHIELD_MAX}）"
            )))
        }
        "mult" | "shield" => Ok(()),
        other => Err(DomainError::Validation(format!(
            "「{name}」effect 只能是 mult | shield，收到「{other}」"
        ))),
    }
}

/// 玩家持有的、可用于猜大小的道具（含持有数）。效果形状坏掉的**不进列表**：
/// 宁可玩家看不到，也不给他一个点了必报错的按钮。
pub(super) async fn held(
    db: &PgPool,
    user_id: i64,
) -> DomainResult<Vec<(Prop, i64)>> {
    let rows: Vec<(String, String, String, String, i64, i64)> = sqlx::query_as(
        "SELECT i.key, i.name, i.icon, i.game_effect->>'effect', \
                (i.game_effect->>'value')::bigint, h.held \
           FROM arcade_items i \
           JOIN arcade_item_held h \
             ON h.item_key = i.key AND h.user_id = $1 \
          WHERE i.enabled AND i.use_kind = 'game' \
            AND i.game_effect->>'game' = 'bigsmall' \
            AND h.held > 0 \
          ORDER BY i.sort, i.key",
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .map_err(dberr)?;
    let mut out = Vec::new();
    for (key, name, icon, effect, value, h) in rows {
        let shape = serde_json::json!({
            "game": "bigsmall", "effect": effect, "value": value,
        });
        if check_effect_shape(&Some(shape), &name).is_ok() {
            out.push((
                Prop {
                    key,
                    name,
                    icon,
                    effect,
                    value,
                },
                h,
            ));
        }
    }
    Ok(out)
}

/// 校验这一局要挂哪些道具：必须在背包里、且同一类效果至多一件（不叠乘）。
pub(super) async fn pick(
    db: &PgPool,
    user_id: i64,
    keys: &[String],
) -> DomainResult<Vec<Prop>> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let avail = held(db, user_id).await?;
    let mut picked: Vec<Prop> = Vec::new();
    for k in keys {
        let Some((p, n)) = avail.iter().find(|(p, _)| &p.key == k) else {
            return Err(DomainError::Validation(format!(
                "道具「{k}」不可用：不在背包里，或不是本玩法的道具"
            )));
        };
        if *n < 1 {
            return Err(DomainError::Validation(format!(
                "道具「{}」已用完",
                p.name
            )));
        }
        if picked.iter().any(|x| x.key == p.key) {
            return Err(DomainError::Validation(format!(
                "道具「{}」重复挂载",
                p.name
            )));
        }
        if picked.iter().any(|x| x.effect == p.effect) {
            return Err(DomainError::Validation(format!(
                "同一类效果每局只能挂一件（{}）",
                p.effect
            )));
        }
        picked.push(p.clone());
    }
    Ok(picked)
}

/// 消耗一件道具（幂等）：锁用户行后再判持有，防并发两局抢最后一件。
pub(super) async fn consume(
    db: &PgPool,
    user_id: i64,
    key: &str,
    game: &str,
    idem: &str,
) -> DomainResult<()> {
    let mut tx = db.begin().await.map_err(dberr)?;
    let held: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT held FROM arcade_item_held \
                           WHERE user_id = $1 AND item_key = $2), 0)::bigint \
           FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .bind(key)
    .fetch_one(&mut *tx)
    .await
    .map_err(dberr)?;
    if held < 1 {
        tx.rollback().await.map_err(dberr)?;
        return Err(DomainError::Validation(
            "道具已被用完，请刷新后重试".into(),
        ));
    }
    let res = sqlx::query(
        "INSERT INTO arcade_item_uses \
             (item_key, user_id, qty, use_kind, game, idem) \
         VALUES ($1, $2, 1, 'game', $3, $4)",
    )
    .bind(key)
    .bind(user_id)
    .bind(game)
    .bind(idem)
    .execute(&mut *tx)
    .await;
    if let Err(e) = res {
        // 同键重放（这一笔早受理）不算错，也不重复扣件
        let replay = e
            .as_database_error()
            .map(|d| d.constraint().unwrap_or("").contains("idem"))
            .unwrap_or(false);
        if replay {
            tx.commit().await.map_err(dberr)?;
            return Ok(());
        }
        return Err(dberr(e));
    }
    tx.commit().await.map_err(dberr)?;
    Ok(())
}

/// 道具对本局的加权结果。
pub(super) struct Applied {
    /// 额外要入账的魔力（倍率增量 + 护盾返还）
    pub extra: i64,
    pub shield_refund: i64,
    /// 生效后的等效倍率（千分），仅赢局且挂了倍率券时给出
    pub effective_mult: Option<i64>,
}

/// 结算加权（纯函数）：倍率只在赢生效、护盾只在输生效 —— 两者必然互斥，
/// 所以一局至多贡献一份加成，不会出现「又翻倍又返还」的叠加。
pub(super) fn apply(
    props: &[Prop],
    bet: i64,
    side: &str,
    base_payout: i64,
    win_permille: i64,
) -> Applied {
    let mut extra = 0i64;
    let mut shield_refund = 0i64;
    let mut effective_mult = None;
    for p in props {
        match p.effect.as_str() {
            "mult" if side == "win" => {
                let total = base_payout.saturating_mul(p.value) / 1000;
                extra += (total - base_payout).max(0);
                effective_mult =
                    Some(win_permille.saturating_mul(p.value) / 1000);
            }
            "shield" if side == "lose" => {
                let r = bet.saturating_mul(p.value) / 1000;
                extra += r;
                shield_refund += r;
            }
            _ => {}
        }
    }
    Applied {
        extra,
        shield_refund,
        effective_mult,
    }
}
