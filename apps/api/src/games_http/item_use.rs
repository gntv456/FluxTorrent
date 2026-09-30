//! 物品的「用途」侧：奖品是站内虚拟物品，抽到之后必须能在站内用掉。
//!
//! 三条用途（与 0246 的 use_kind 一一对应）：
//!  · collect —— 收藏件，没有可执行用途：**用就是报错**，不静默吞掉一次点击；
//!  · spark   —— 按权威折算价 anchor 兑现成魔力。这份负债在奖池 EV 里已按 anchor
//!              计入，兑现只是实现时点不同，不新增发行；
//!  · sku     —— 复用商店生效链 `apply_item_effect`（装扮 / 券 / 上传量 / VIP
//!              都在那边实现）。娱乐屋**不再另写一份效果**，否则就是第二份清单，
//!              两处实现必然漂移。
//!
//! 持有数不另存：`arcade_item_held` 视图按「发放账 − 消耗账」反推，
//! 与 0244 的全服余量视图同一口径。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;

use super::helpers::idem_key;
use super::pool::dberr;
use crate::dto::ok;
use crate::economy_http::{
    apply_item_effect, earn_spark_tx, has_effect, SpendOutcome,
};
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 空串按 collect 处理：老客户端不带这两个字段时，行为与 0244 时代完全一致
pub(super) fn use_kind_or_collect(k: &str) -> &'static str {
    match k {
        "spark" => "spark",
        "sku" => "sku",
        _ => "collect",
    }
}

/// 绑定的商店 SKU：(名称, kind, config, 售价, 在售)
pub(super) async fn bound_sku(
    db: &PgPool,
    use_ref: &str,
) -> DomainResult<Option<(String, String, serde_json::Value, i64, bool)>> {
    let id: i64 = use_ref.trim().parse().map_err(|_| {
        DomainError::Validation(
            "绑定商店 SKU 要在 use_ref 里填 SKU 编号（整数）".into(),
        )
    })?;
    let row =
        sqlx::query_as::<_, (String, String, serde_json::Value, i64, bool)>(
            "SELECT name, kind, config, price, active \
           FROM shop_items WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(db)
        .await
        .map_err(dberr)?;
    Ok(row)
}

/// 写侧形制闸：用途配错了必须在保存时就拦住，而不是等玩家点「使用」才发现。
pub(super) async fn check_use_shape(
    db: &PgPool,
    use_kind: &str,
    use_ref: &str,
    anchor: i64,
    name: &str,
) -> DomainResult<()> {
    match use_kind_or_collect(use_kind) {
        "spark" => {
            if anchor <= 0 {
                return Err(DomainError::Validation(format!(
                    "「{name}」选了「兑现魔力」但折算价为 {anchor}：\
                     兑现等于凭空印钞，anchor 必须为正"
                )));
            }
        }
        "sku" => {
            let sku = bound_sku(db, use_ref).await?;
            let (sname, skind, _cfg, price, active) = sku.ok_or_else(|| {
                DomainError::Validation(format!(
                    "「{name}」绑定的 SKU「{use_ref}」在商店里不存在：\
                     绑不到东西的奖品就是一张空气"
                ))
            })?;
            if !active {
                return Err(DomainError::Validation(format!(
                    "「{name}」绑的「{sname}」已下架，使用时无处生效：换一个在售 SKU"
                )));
            }
            // 生效链里没有这一类 = 扣了物品什么都不做（审计 P1 的「花钱买空气」）
            if !has_effect(&skind) {
                return Err(DomainError::Validation(format!(
                    "「{sname}」（kind={skind}）没有已实现的生效逻辑，不能绑为奖品用途"
                )));
            }
            // 奖池按 anchor 计负债，实际兑出去的是售价 price 的东西：
            // anchor 低于 price 就是这条配置在偷偷少计
            if anchor < price {
                return Err(DomainError::Validation(format!(
                    "「{name}」折算价 {anchor} 低于绑定「{sname}」的售价 {price}：\
                     奖池会按 anchor 少计负债，把 anchor 提到 {price} 以上或换便宜的 SKU"
                )));
            }
        }
        _ => {}
    }
    Ok(())
}

#[derive(Deserialize)]
struct UseReq {
    item_key: String,
    #[serde(default = "default_qty")]
    qty: i32,
    /// 客户端幂等键：重复提交不再扣一件，也不重复生效
    #[serde(default)]
    idempotency_key: Option<String>,
}

fn default_qty() -> i32 {
    1
}

/// 使用背包里的一件物品。扣减与生效的口径见文件头。
#[post("/games/arcade/backpack/use")]
pub(super) async fn backpack_use(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<UseReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let b = body.into_inner();
    let db = &state.repo.db;
    if b.item_key.trim().is_empty() {
        return Err(DomainError::Validation("item_key 不能为空".into()));
    }
    if b.qty <= 0 || b.qty > 99 {
        return Err(DomainError::Validation("单次使用数量限 1~99".into()));
    }

    let item =
        sqlx::query_as::<_, (String, String, String, String, i64, bool)>(
            "SELECT name, kind, use_kind, use_ref, anchor, enabled \
           FROM arcade_items WHERE key = $1",
        )
        .bind(b.item_key.trim())
        .fetch_optional(db)
        .await
        .map_err(dberr)?;
    let (name, kind, use_kind, use_ref, anchor, enabled) =
        item.ok_or_else(|| {
            DomainError::Validation(format!("目录里没有「{}」", b.item_key))
        })?;
    if !enabled {
        return Err(DomainError::Validation(format!(
            "「{name}」已停用，无法使用"
        )));
    }
    let use_kind = use_kind_or_collect(&use_kind);
    if use_kind == "collect" {
        return Err(DomainError::Validation(format!(
            "「{name}」是收藏件：没有可执行用途（想让它能用，去物品目录给它指定用途）"
        )));
    }
    // sku 类先验绑定，别把「扣了物品却无处生效」留给玩家
    let sku = if use_kind == "sku" {
        let s = bound_sku(db, &use_ref).await?.ok_or_else(|| {
            DomainError::Validation(format!(
                "「{name}」绑定的 SKU「{use_ref}」已不存在，暂时无法使用"
            ))
        })?;
        if !s.4 {
            return Err(DomainError::Validation(format!(
                "「{name}」绑的「{}」已下架，暂时无法使用",
                s.0
            )));
        }
        Some(s)
    } else {
        None
    };

    let idem = idem_key("arcade-use", auth.id, &b.idempotency_key);
    let mut tx = db.begin().await.map_err(dberr)?;
    // 锁用户行：并发两笔「同一件最后一份」必须串行，否则各自看到 held=1
    let held: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT held FROM arcade_item_held \
                           WHERE user_id = $1 AND item_key = $2), 0)::bigint \
           FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(auth.id)
    .bind(b.item_key.trim())
    .fetch_one(&mut *tx)
    .await
    .map_err(dberr)?;
    if held < i64::from(b.qty) {
        drop(tx);
        return Err(DomainError::Validation(format!(
            "背包里「{name}」只有 {held} 件，不够用 {} 件",
            b.qty
        )));
    }
    sqlx::query(
        "INSERT INTO arcade_item_uses (item_key, user_id, qty, use_kind, idem) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(b.item_key.trim())
    .bind(auth.id)
    .bind(b.qty)
    .bind(use_kind)
    .bind(&idem)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        // 同键重放撞 UNIQUE：不是错误，告诉客户端「这一笔早已受理」
        if e.as_database_error()
            .map(|d| d.constraint().unwrap_or("").contains("idem"))
            .unwrap_or(false)
        {
            DomainError::Validation("本笔已受理，请勿重复提交".into())
        } else {
            dberr(e)
        }
    })?;

    let mut spark = 0i64;
    if use_kind == "spark" {
        // 扣件与入账同一个事务：不存在「物品烧了、魔力没到」的中间态
        spark = anchor * i64::from(b.qty);
        let out = earn_spark_tx(
            &mut tx,
            auth.id,
            spark,
            "arcade",
            &format!("arcade-use-spark:{idem}"),
        )
        .await?;
        // Replayed = 这个兑现键早就入过账，而扣件这一笔却是新的。
        // 整笔失败而不是「就当成功」：否则物品白烧，玩家什么也没拿到。
        if matches!(out, SpendOutcome::Replayed) {
            let _ = tx.rollback().await;
            return Err(DomainError::Validation(
                "兑现键与既有流水冲突，本笔未受理，请重新提交".into(),
            ));
        }
    }
    tx.commit().await.map_err(dberr)?;

    // SKU 生效链是商店侧的独立事务（装扮入库 / 券 / 时长），放在扣件之后：
    // 宁可「已扣但需人工补」，也不给「未扣先生效」留口子。
    if let Some((sname, skind, mut cfg, _, _)) = sku {
        let id: i64 = use_ref.trim().parse().unwrap_or(0);
        cfg["item_id"] = json!(id);
        for _ in 0..b.qty {
            // 归属记 `prize`：这条链是「用掉一件奖品」，与商店买入不是一回事
            apply_item_effect(db, auth.id, &skind, &cfg, "prize").await?;
        }
        state
            .repo
            .audit(Some(auth.id), "arcade.item.use.sku", None)
            .await;
        return Ok(ok(json!({
            "key": b.item_key, "name": name, "kind": kind,
            "use_kind": use_kind, "qty": b.qty,
            "granted_as": sname, "spark": 0,
            "held": held - i64::from(b.qty),
        })));
    }
    state
        .repo
        .audit(Some(auth.id), "arcade.item.use", None)
        .await;
    Ok(ok(json!({
        "key": b.item_key, "name": name, "kind": kind,
        "use_kind": use_kind, "qty": b.qty,
        "spark": spark, "held": held - i64::from(b.qty),
    })))
}
