//! 批量发放的 medal/item 分支（0286 从 increment_grant.rs 再拆守 300 行门禁）。
//! 语义与单发路径对齐：勋章 PK 冲突跳过；道具按 kind 分流即时/背包/券。
//!
//! 0291 两条口径收紧：
//! - 生效值改读 `economy_http` 的三个 config 取值器，与商店购买同一把尺。
//!   此前批量读 `config.amount`、购买读 `config.spark`，站里那张
//!   `{"spark":1000}` 的魔力卡批量发出去是「每人 +0」而 affected 照报人数。
//! - 全部写操作在调用方给的那一批事务里执行，库存占位与结果同生同死。

use super::increment_grant::{bulk_backpack_orders, GrantCtx, Tx};
use super::increment_stock::stock_take;
use crate::economy_http::{credit_gb, spark_amount, voucher_kind};
use crate::errors::{DomainError, DomainResult};

/// medal：每人 1 枚（source='admin'、有效期随 medals.duration_days、
/// PK 冲突跳过天然幂等）。等级护栏：跳过不低于操作者的目标。
pub(super) async fn grant_medal(
    ctx: &GrantCtx<'_>,
    tx: &mut Tx<'_>,
    chunk: &[i64],
    medal_id: i64,
    days: Option<i32>,
) -> DomainResult<u64> {
    let mut affected: u64 = 0;
    for uid in chunk {
        let skipped: bool = sqlx::query_scalar(
            "SELECT class_id >= $2 FROM users WHERE id = $1",
        )
        .bind(uid)
        .bind(ctx.auth.class_id)
        .fetch_one(&mut **tx)
        .await
        .unwrap_or(true);
        if skipped {
            continue;
        }
        let n = sqlx::query(
            "INSERT INTO user_medals (user_id, medal_id, source, expires_at) \
             SELECT $1, $2, 'admin', CASE WHEN $3::int IS NOT NULL \
                    THEN now() + make_interval(days => $3::int) \
                    ELSE now() + make_interval(days => m.duration_days) END \
             FROM medals m WHERE m.id = $2 \
             ON CONFLICT (user_id, medal_id) DO NOTHING",
        )
        .bind(uid)
        .bind(medal_id)
        .bind(days)
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
        affected += n;
    }
    Ok(affected)
}

/// item：按道具 kind 分流——即时类直接生效；背包类入包；
/// 券类入 user_vouchers（与 shop_effects 购买路径同构）。
pub(super) async fn grant_item(
    ctx: &GrantCtx<'_>,
    tx: &mut Tx<'_>,
    chunk: &[i64],
    item_id: i64,
) -> DomainResult<u64> {
    let item: Option<(String, serde_json::Value)> = sqlx::query_as(
        "SELECT kind, config FROM shop_items WHERE id = $1 AND active = true",
    )
    .bind(item_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((kind, config)) = item else {
        return Err(DomainError::NotFound(item_id));
    };
    // 即时经济类与单发同闸：批量侧的权限位在 increment_targets 里按 kind 判，
    // 这里判的是「这件 SKU 到底能不能算出一个正数效果」
    if kind == "gift_spark" && spark_amount(&config).is_none() {
        return Err(DomainError::Validation(
            "该道具配置里没有可用的魔力数额（spark），已拒绝发放".into(),
        ));
    }
    if kind == "upload_credit" && credit_gb(&config).is_none() {
        return Err(DomainError::Validation(
            "该道具配置里没有可用的 GB 数（gb），已拒绝发放".into(),
        ));
    }
    stock_take(tx, item_id, ctx.amount * chunk.len() as i64).await?;
    let n = match kind.as_str() {
        "invite" | "temp_invite" => sqlx::query(
            "UPDATE users SET quota_extra = quota_extra + $2 \
             WHERE id = ANY($1) AND status < 2",
        )
        .bind(chunk)
        .bind(ctx.amount)
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected(),
        "upload_credit" => {
            let gb = credit_gb(&config).unwrap_or(0);
            super::bulk_traffic::grant_upload_credit_tx(
                tx,
                chunk,
                gb,
                ctx.amount,
                ctx.auth.id,
                ctx.batch_id,
            )
            .await?
        }
        "gift_spark" => {
            let per = spark_amount(&config).unwrap_or(0) * ctx.amount;
            let mut affected: u64 = 0;
            for uid in chunk {
                let idem = format!(
                    "increment_bulk:{}:{uid}:{}",
                    ctx.batch_id,
                    uuid::Uuid::new_v4().simple()
                );
                crate::economy_http::expect_spent(
                    crate::economy_http::earn_spark_tx(
                        tx, *uid, per, "increment_bulk", &idem,
                    )
                    .await?,
                )?;
                affected += 1;
            }
            affected
        }
        // 券类（0286 P1）：库存语义是 user_vouchers 行——default 分支塞
        // shop_orders 会发成死券（/me/vouchers 读不到、无从核销）
        "voucher_free" | "voucher_neutral" => {
            let vkind = voucher_kind(&config, &kind);
            let mut affected: u64 = 0;
            for uid in chunk {
                for _ in 0..ctx.amount {
                    sqlx::query(
                        "INSERT INTO user_vouchers \
                         (user_id, kind, source) VALUES ($1, $2, 'admin')",
                    )
                    .bind(uid)
                    .bind(&vkind)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                    affected += 1;
                }
            }
            affected
        }
        _ => bulk_backpack_orders(ctx, tx, chunk, item_id, &config).await?,
    };
    Ok(n)
}
