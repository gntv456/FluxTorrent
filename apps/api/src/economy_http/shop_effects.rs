//! 道具生效逻辑（M11）—— **薄壳，真身在 `flux-shared`**。
//!
//! 生效链原先只在本文件里，于是 worker 的 `shop_fulfill` 补发不了效果：
//! 崩溃后卡在冻结态的订单只能退款。现在抽到 `crates/flux-shared`，
//! 两边引用同一份源码——既不把 worker 耦到 api 的 actix 栈上，
//! 也不会出现「api 发了券、worker 退了款」这种漂移。
//!
//! 本文件只做两件事：把 api 的 `DomainError` 与共享层的 `EffectError`
//! 互转，以及重新导出那几个纯函数（`has_effect` / `spark_amount` /
//! `credit_gb` / `voucher_kind`）保持既有调用点不变。

use serde_json::Value;
use sqlx::PgPool;

pub use flux_shared::{credit_gb, has_effect, spark_amount, voucher_kind};

use crate::errors::{DomainError, DomainResult};

/// 共享层错误 → api 错误码。
fn map(e: flux_shared::EffectError) -> DomainError {
    match e {
        flux_shared::EffectError::Db(d) => DomainError::Internal(d.into()),
        flux_shared::EffectError::Internal(m) => {
            DomainError::Internal(anyhow::anyhow!(m))
        }
    }
}

/// 授予一件 SKU 的效果。
pub async fn apply_item_effect(
    db: &PgPool,
    user_id: i64,
    kind: &str,
    config: &Value,
    dressup_source: &str,
) -> DomainResult<()> {
    flux_shared::apply_item_effect(db, user_id, kind, config, dressup_source)
        .await
        .map_err(map)
}
