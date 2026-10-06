//! 批量发放上传量（0285 从 increment_bulk.rs 拆出）。
//!
//! 拆出理由有二：
//! 1. `increment_bulk.rs` 顶在行数基线上，而这条分支必须变长——按门禁口径拆文件；
//! 2. 发放量**必须与流水同写**。`users.uploaded` 只是「balance_baseline +
//!    SUM(traffic_ledger)」的展示快照，announce 消费轮每次都会重算覆盖
//!    （P0-2 实测：只 UPDATE 快照的 1GB 补偿，用户一次普通 announce 后归零）。
//!
//! 0291：改成接调用方的事务（`_tx`）。原来它自己 begin/commit，被批量发放分支
//! 调用时嵌在外层事务里就跑不了；更重要的是流水要带 reason + operator_id +
//! batch 锚点，否则事后读不出「这批是谁在哪一次发放里补的」。

use crate::errors::{DomainError, DomainResult};

/// 给一批用户各发放 `gb × amount` GiB 上传量（与调用方同事务）。
/// 返回实际覆盖的用户数。
pub(crate) async fn grant_upload_credit_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ids: &[i64],
    gb: i64,
    amount: i64,
    operator: i64,
    batch_id: &str,
) -> DomainResult<u64> {
    let total_gb = gb * amount;
    sqlx::query(
        "INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, \
         delta_down, window_start, reason, operator_id) \
         SELECT nextval('traffic_ledger_id_seq'), u.id, NULL, \
                $2::bigint * 1073741824, 0, now(), $3, $4 \
         FROM unnest($1::bigint[]) AS x(uid) \
         JOIN users u ON u.id = x.uid WHERE u.status < 2",
    )
    .bind(ids)
    .bind(total_gb)
    .bind(format!(
        "bulk:{} x{total_gb}GB",
        &batch_id[..8.min(batch_id.len())]
    ))
    .bind(operator)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let n = sqlx::query(
        "UPDATE users SET uploaded = uploaded + $2::bigint * 1073741824 \
         WHERE id = ANY($1) AND status < 2",
    )
    .bind(ids)
    .bind(total_gb)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    Ok(n)
}
