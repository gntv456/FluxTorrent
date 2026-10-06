//! 批量发放上传量（0285 从 increment_bulk.rs 拆出）。
//!
//! 拆出理由有二：
//! 1. `increment_bulk.rs` 顶在行数基线上，而这条分支必须变长——按门禁口径拆文件；
//! 2. 发放量**必须与流水同写**。`users.uploaded` 只是「balance_baseline +
//!    SUM(traffic_ledger)」的展示快照，announce 消费轮每次都会重算覆盖
//!    （P0-2 实测：只 UPDATE 快照的 1GB 补偿，用户一次普通 announce 后归零）。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 给一批用户各发放 `gb_per_user × amount` GiB 上传量。
/// 返回实际覆盖的用户数（与旧 `UPDATE ... rows_affected` 口径一致）。
pub(crate) async fn grant_upload_credit(
    db: &PgPool,
    ids: &[i64],
    gb: i64,
    amount: i64,
    operator: i64,
) -> DomainResult<u64> {
    let total_gb = gb * amount;
    let mut tx = db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
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
    .bind(format!("bulk_upload_credit x{total_gb}GB"))
    .bind(operator)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let n = sqlx::query(
        "UPDATE users SET uploaded = uploaded + $2::bigint * 1073741824 \
         WHERE id = ANY($1) AND status < 2",
    )
    .bind(ids)
    .bind(total_gb)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(n)
}
