//! 商城三段式履约的 SQL 唯一居所（FRAME §2 H 层不下沉 sqlx）。
//!
//! 消费方：`economy_http/shop.rs`（下单冻结 / 履约后结算）。
//! 状态机判定在 `domain::shop`（冻结→履约→扣款），本文件只读写。
//!
//! 【为什么冻结与台账必须同事务】既有路径是「扣款 → 发效果」，中间崩溃
//! 就是永久的「花钱买空气」。0383 把钱与效果之间那段窗口变成可回滚的
//! 冻结态；订单与冻结台账若分两次写，中途失败会留下「订单有、冻结台账
//! 没有」的对账缺口——那比原先更难查。故本文件的两条 INSERT 共用一个事务。

use super::Repo;
use crate::errors::{DomainError, DomainResult};

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

/// 建订单并同时落冻结台账（0383 三段式第一步：冻结）。
///
/// 幂等：`ON CONFLICT (idempotency_key) DO NOTHING`，冲突（重放）时
/// **不**写台账——已有那单自带台账，再写就成了双份冻结，对账会多出钱。
pub async fn freeze_order(
    repo: &Repo,
    user_id: i64,
    item_id: i64,
    price: i64,
    idem: &str,
    config: &serde_json::Value,
    frozen_amount: i64,
) -> DomainResult<()> {
    let mut tx = repo.db.begin().await.map_err(internal)?;
    let oid: Option<i64> = sqlx::query_scalar(
        "INSERT INTO shop_orders \
         (user_id, item_id, price, idempotency_key, config_snapshot, \
          fulfill_state, frozen_amount, frozen_at) \
         VALUES ($1, $2, $3, $4, $5, 'frozen', $6, now()) \
         ON CONFLICT (idempotency_key) DO NOTHING \
         RETURNING id",
    )
    .bind(user_id)
    .bind(item_id)
    .bind(price)
    .bind(idem)
    .bind(config.clone())
    .bind(frozen_amount)
    .fetch_optional(&mut *tx)
    .await
    .map_err(internal)?;
    if let Some(oid) = oid {
        sqlx::query(
            "INSERT INTO shop_freezes (order_id, user_id, amount, state) \
             VALUES ($1, $2, $3, 'frozen') ON CONFLICT (order_id) DO NOTHING",
        )
        .bind(oid)
        .bind(user_id)
        .bind(frozen_amount)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    }
    tx.commit().await.map_err(internal)?;
    Ok(())
}

/// 履约完成：冻结转 consumed、订单转 done（0383 三段式第三步）。
///
/// 幂等靠 `fulfill_state = 'frozen'` 这个 CAS 判据：重复调用第二次
/// 影响 0 行，不报错也不重复记账。卡牌类多行订单一次推全部。
pub async fn settle_freeze(
    repo: &Repo,
    user_id: i64,
    idem: &str,
) -> DomainResult<()> {
    sqlx::query(
        "UPDATE shop_orders SET fulfill_state = 'done', fulfilled_at = now() \
         WHERE user_id = $1 \
           AND (idempotency_key = $2 OR idempotency_key LIKE $3) \
           AND fulfill_state = 'frozen'",
    )
    .bind(user_id)
    .bind(idem)
    .bind(format!("{}:%", idem))
    .execute(&repo.db)
    .await
    .map_err(internal)?;
    sqlx::query(
        "UPDATE shop_freezes f SET state = 'consumed', settled_at = now() \
         FROM shop_orders o \
         WHERE o.id = f.order_id AND o.user_id = $1 \
           AND (o.idempotency_key = $2 OR o.idempotency_key LIKE $3) \
           AND f.state = 'frozen'",
    )
    .bind(user_id)
    .bind(idem)
    .bind(format!("{}:%", idem))
    .execute(&repo.db)
    .await
    .map_err(internal)?;
    Ok(())
}

/// 取价行（0383 递增价格所需的三列一并带上）。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ItemRow {
    pub name: String,
    pub kind: String,
    pub price: i64,
    pub config: serde_json::Value,
    pub price_step: Option<i64>,
    pub price_max: Option<i64>,
    pub price_anchor_qty: i32,
    pub sold: i64,
}

/// 读一个在售道具（含递增价格档位与已售量）。
pub async fn item_for_buy(
    repo: &Repo,
    item_id: i64,
) -> DomainResult<Option<ItemRow>> {
    Ok(sqlx::query_as(
        "SELECT name, kind, price, config, \
                price_step, price_max, price_anchor_qty, \
                COALESCE(stock_used, 0) AS sold \
         FROM shop_items WHERE id = $1 AND active = true",
    )
    .bind(item_id)
    .fetch_optional(&repo.db)
    .await
    .map_err(internal)?)
}
