//! 批量发放的库存口径（0291 从 increment_grant.rs 拆出守 300 行门禁）。
//! 这一份只管一件事：**限量道具的配额怎么占、怎么读、发不出去时报什么**。
//!
//! 拆出来还有一个实际理由：试运行体检（increment_bulk.rs）与真实发放
//! （increment_grant.rs）必须查同一件 SKU、用同一把尺。占位与发放分在两个
//! 函数里各写一遍 SQL，是「体检过了真发却撞闸」这类假绿的来源。

use super::increment_bulk::IncrementBulkReq;
use super::increment_grant::{dberr, Tx};

use crate::errors::{DomainError, DomainResult};

/// 库存配额（0287 P3）：quota 非空时在**当前事务内**原子占位。
/// 越界即拒，外层回滚——不再手工减回去。
pub(super) async fn stock_take(
    tx: &mut Tx<'_>,
    item_id: i64,
    qty: i64,
) -> DomainResult<()> {
    let within: Option<bool> = sqlx::query_scalar(
        "UPDATE shop_items SET stock_used = stock_used + $2 \
         WHERE id = $1 AND stock_quota IS NOT NULL \
         RETURNING (stock_used <= stock_quota)",
    )
    .bind(item_id)
    .bind(qty)
    .fetch_optional(&mut **tx)
    .await
    .map_err(dberr)?;
    if within == Some(false) {
        return Err(DomainError::Validation(
            "该道具库存已发罄（stock_quota）".into(),
        ));
    }
    Ok(())
}

/// 补签卡用的到底是哪件 SKU：字典名 makeup_card 优先，历史名 resub_card 兼容。
/// 两处（真发 / 试运行体检）必须走同一个解析，否则体检的是另一件东西。
pub(super) async fn resub_item_id(db: &sqlx::PgPool) -> DomainResult<i64> {
    let id: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM shop_items WHERE kind IN ('makeup_card','resub_card') \
         AND active = true ORDER BY kind = 'makeup_card' DESC, id LIMIT 1",
    )
    .fetch_optional(db)
    .await
    .map_err(dberr)?;
    id.ok_or_else(|| {
        DomainError::Validation(
            "商店缺少可用的补签卡道具（makeup_card）".into(),
        )
    })
}

/// 这一批到底要占哪件 SKU 的库存（不需要库存的 kind 返回 None）。
pub(super) async fn stock_item_id(
    db: &sqlx::PgPool,
    body: &IncrementBulkReq,
) -> DomainResult<Option<i64>> {
    Ok(match body.kind.as_str() {
        "item" => Some(
            body.item_id
                .ok_or_else(|| DomainError::Validation("需选择道具".into()))?,
        ),
        "resub_card" => Some(resub_item_id(db).await?),
        _ => None,
    })
}

/// 库存体检读数 (已用, 配额)；配额 None = 不限量。
pub(super) async fn stock_read(
    db: &sqlx::PgPool,
    item_id: i64,
) -> DomainResult<(i64, Option<i64>)> {
    let row: Option<(i64, Option<i64>)> = sqlx::query_as(
        "SELECT stock_used, stock_quota FROM shop_items \
         WHERE id = $1 AND active = true",
    )
    .bind(item_id)
    .fetch_optional(db)
    .await
    .map_err(dberr)?;
    row.ok_or(DomainError::NotFound(item_id))
}
