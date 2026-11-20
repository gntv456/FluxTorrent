//! 任务商店兑换仓储层（五问评审 0115）。
//! SQL 全部沉在这里，HTTP 层只做编排——遵守 H 层禁直写 sqlx 的纪律。

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ShopItem {
    pub id: i64,
    pub name: String,
    pub span: String,
    pub kind: Option<String>,
    pub config: Value,
    pub require_tier: String,
    pub require_count: i32,
    pub cost: f64,
    pub stock: i32,
    pub stock_used: i32,
}

/// 兑换商品（含发放配置）。不存在返回 None。
pub async fn item_for_buy(
    state: &AppState,
    item_id: i64,
) -> DomainResult<Option<ShopItem>> {
    let row: Option<(
        i64,
        String,
        String,
        Option<String>,
        Value,
        String,
        i32,
        f64,
        i32,
        i32,
    )> = sqlx::query_as(
        "SELECT id, name, span, kind, config, require_tier, require_count, \
             cost::float8, stock, stock_used \
             FROM task_shop_items WHERE id = $1",
    )
    .bind(item_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(row.map(
        |(
            id,
            name,
            span,
            kind,
            config,
            require_tier,
            require_count,
            cost,
            stock,
            stock_used,
        )| ShopItem {
            id,
            name,
            span,
            kind,
            config,
            require_tier,
            require_count,
            cost,
            stock,
            stock_used,
        },
    ))
}

/// 用户在指定难度档已完成任务数（门槛判据）。
pub async fn tier_done_count(
    state: &AppState,
    user_id: i64,
    tier: &str,
) -> DomainResult<i64> {
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM task_claims c \
         JOIN tasks t ON t.id = c.task_id \
         WHERE c.user_id = $1 AND t.tier = $2 AND c.status = 1",
    )
    .bind(user_id)
    .bind(tier)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(n)
}

/// 原子占位库存：占位成功返回 true，售罄返回 false。
/// stock <= 0 视为不限量（历史种子数据均为正整数，留语义给运营）。
pub async fn take_stock(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    item_id: i64,
) -> DomainResult<bool> {
    let ok: Option<bool> = sqlx::query_scalar(
        "UPDATE task_shop_items SET stock_used = stock_used + 1 \
         WHERE id = $1 AND (stock <= 0 OR stock_used < stock) \
         RETURNING true",
    )
    .bind(item_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok.is_some())
}

/// 归还占位（spend 判重放或后续发券失败时）。
pub async fn release_stock(
    db: &sqlx::PgPool,
    item_id: i64,
) -> DomainResult<()> {
    sqlx::query(
        "UPDATE task_shop_items SET stock_used = GREATEST(0, stock_used - 1) \
         WHERE id = $1",
    )
    .bind(item_id)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 记录兑换订单（effect_applied=false，待发放）。
pub async fn insert_order(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    item_id: i64,
    cost: i64,
    idem: &str,
) -> DomainResult<i64> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO task_shop_orders \
         (user_id, item_id, cost, idempotency_key) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(user_id)
    .bind(item_id)
    .bind(cost)
    .bind(idem)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            DomainError::Validation("该笔兑换已受理，请勿重复提交".into())
        }
        other => DomainError::Internal(other.into()),
    })?;
    Ok(id)
}

/// 按幂等键查已有订单（重放补发用）：返回 (order_id, effect_applied, item_id, cost)。
pub async fn order_by_idem(
    state: &AppState,
    user_id: i64,
    idem: &str,
) -> DomainResult<Option<(i64, bool, i64, i64)>> {
    let row: Option<(i64, bool, i64, i64)> = sqlx::query_as(
        "SELECT id, effect_applied, item_id, cost FROM task_shop_orders \
         WHERE user_id = $1 AND idempotency_key = $2",
    )
    .bind(user_id)
    .bind(idem)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(row)
}

/// CAS 标记已发放（只翻 false→true，防重复补发）。
pub async fn mark_effect_applied(
    state: &AppState,
    order_id: i64,
) -> DomainResult<()> {
    sqlx::query(
        "UPDATE task_shop_orders SET effect_applied = true \
         WHERE id = $1 AND NOT effect_applied",
    )
    .bind(order_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}
