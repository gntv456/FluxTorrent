//! 抽卡碎片快照的共用原语（拆自 gacha_http/craft.rs）。
//!
//! 为什么单独一个文件：碎片余额行 `gacha_shard_balance` 原先只有抽卡路径会
//! 建，而分解/兑换/升级三条口共用的 `shard_move` 直接 `SELECT … FOR UPDATE`
//! 假设它存在（P-353：只在流水里有碎片的会员一进消费口就 500）。建行与播种
//! 值这个「谁都会踩」的形状必须有唯一一处实现，否则每处都会各自长出来。

use crate::errors::{DomainError, DomainResult};

/// 保证该会员有碎片快照行，并按**流水净额**播种。
///
/// 快照以流水为准是本仓既有口径（`traffic_ledger` 的 6h reconcile 同理）：
/// 按 0 播种会把只在 `gacha_shard_ledger` 里的历史碎片静默吞掉（真库 A/B：
/// 按 0 播种余额 0、按 sum(delta) 播种余额 100，且「余额≠流水」人数 1→0）。
pub(super) async fn ensure_shard_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
) -> DomainResult<()> {
    sqlx::query(
        "INSERT INTO gacha_shard_balance (user_id, balance) \
         SELECT $1, COALESCE((SELECT sum(delta) FROM gacha_shard_ledger \
                              WHERE user_id = $1), 0) \
         ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 读快照（行必须已存在；调用前先 `ensure_shard_row`），并加行锁防并发。
pub(super) async fn lock_shard_balance(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
) -> DomainResult<i32> {
    ensure_shard_row(tx, user_id).await?;
    sqlx::query_scalar(
        "SELECT balance FROM gacha_shard_balance WHERE user_id = $1 \
         FOR UPDATE",
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}
