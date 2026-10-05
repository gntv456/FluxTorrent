//! 批量发放的 medal/item 分支（0286 从 increment_grant.rs 再拆守 300 行门禁）。
//! 语义与单发路径对齐：勋章 PK 冲突跳过；道具按 kind 分流即时/背包/券。

use super::increment_grant::{bulk_backpack_orders, GrantCtx};

use crate::errors::{DomainError, DomainResult};

/// medal：每人 1 枚（source='admin'、有效期随 medals.duration_days、
/// PK 冲突跳过天然幂等）。等级护栏：跳过不低于操作者的目标。
pub(super) async fn grant_medal(
    ctx: &GrantCtx<'_>,
    chunk: &[i64],
    medal_id: i64,
) -> DomainResult<u64> {
    let mut affected: u64 = 0;
    for uid in chunk {
        let skipped: bool = sqlx::query_scalar(
            "SELECT class_id >= $2 FROM users WHERE id = $1",
        )
        .bind(uid)
        .bind(ctx.auth.class_id)
        .fetch_one(ctx.db)
        .await
        .unwrap_or(true);
        if skipped {
            continue;
        }
        let n = sqlx::query(
            "INSERT INTO user_medals (user_id, medal_id, source, expires_at) \
             SELECT $1, $2, 'admin', \
                    now() + make_interval(days => m.duration_days) \
             FROM medals m WHERE m.id = $2 \
             ON CONFLICT (user_id, medal_id) DO NOTHING",
        )
        .bind(uid)
        .bind(medal_id)
        .execute(ctx.db)
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
    chunk: &[i64],
    item_id: i64,
) -> DomainResult<u64> {
    let item: Option<(String, serde_json::Value)> = sqlx::query_as(
        "SELECT kind, config FROM shop_items WHERE id = $1 AND active = true",
    )
    .bind(item_id)
    .fetch_optional(ctx.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((kind, config)) = item else {
        return Err(DomainError::NotFound(item_id));
    };
    let db = ctx.db;
    match kind.as_str() {
        "invite" | "temp_invite" => {
            Ok(sqlx::query(
                "UPDATE users SET quota_extra = quota_extra + $2 \
                 WHERE id = ANY($1) AND status < 2",
            )
            .bind(chunk)
            .bind(ctx.amount)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected())
        }
        "upload_credit" => {
            // 发放与流水同写，见 admin_p3_http::bulk_traffic（P0-2，0285）
            super::bulk_traffic::grant_upload_credit(
                db,
                chunk,
                config.get("gb").and_then(|v| v.as_i64()).unwrap_or(1),
                ctx.amount,
                ctx.auth.id,
            )
            .await
        }
        "gift_spark" => {
            let amt = config
                .get("amount")
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                * ctx.amount;
            let mut affected: u64 = 0;
            for uid in chunk {
                let idem = format!(
                    "increment_bulk:{}:{uid}:{}",
                    ctx.batch_id,
                    uuid::Uuid::new_v4().simple()
                );
                crate::economy_http::earn_spark(
                    db,
                    *uid,
                    amt,
                    "increment_bulk",
                    &idem,
                )
                .await?;
                affected += 1;
            }
            Ok(affected)
        }
        // 券类（0286 P1）：库存语义是 user_vouchers 行——default 分支塞
        // shop_orders 会发成死券（/me/vouchers 读不到、无从核销）
        "voucher_free" | "voucher_neutral" => {
            let vkind = config
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("free");
            let mut affected: u64 = 0;
            for uid in chunk {
                for _ in 0..ctx.amount {
                    sqlx::query(
                        "INSERT INTO user_vouchers \
                         (user_id, kind, source) \
                         VALUES ($1, $2, 'admin')",
                    )
                    .bind(uid)
                    .bind(vkind)
                    .execute(db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                    affected += 1;
                }
            }
            Ok(affected)
        }
        _ => bulk_backpack_orders(ctx, chunk, item_id, &config).await,
    }
}
