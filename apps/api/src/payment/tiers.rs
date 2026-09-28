//! 捐赠回馈档位（E7）：累计实付达档 → 魔力/上传量/邀请/勋章自动发放。

//! 配置存 settings 键 donation_tiers（JSON 数组，空=不启用）；发放幂等双闸——
//! ① settle 侧 pending→paid CLAIM（回调重放天然跳过）② tier_granted CAS 置位
//! （照抄 shop_orders.effect_applied 范式，回调与手动补单两入口共用）。
//! 累计口径：sum(payment_orders.amount_paid WHERE status='paid')——不含模拟
//! 流水与钱包消费，重放不重复计数。

use crate::economy_http::earn_spark_tx;
use crate::errors::{DomainError, DomainResult};

/// 档位定义（settings JSON 单条）
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct DonationTier {
    /// 达档累计实付（USD，含本单）
    pub min_usd: f64,
    /// 展示名（如「铜牌赞助者」）
    pub label: String,
    /// 魔力奖励
    #[serde(default)]
    pub spark: i64,
    /// 上传量奖励（GiB）
    #[serde(default)]
    pub upload_gb: i64,
    /// 邀请码直发数量
    #[serde(default)]
    pub invites: i32,
    /// 档位勋章 id（0/缺省=无）
    #[serde(default)]
    pub medal_id: i64,
}

/// 读档位配置（解析失败=空：回调路径永不被坏配置打断）
pub async fn load_tiers(db: &sqlx::PgPool) -> Vec<DonationTier> {
    let raw: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'donation_tiers'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    let Some(raw) = raw else { return Vec::new() };
    serde_json::from_str(&raw).unwrap_or_else(|e| {
        tracing::warn!(?e, "donation_tiers 配置非法，按无档位处理");
        Vec::new()
    })
}

/// 档位判定：累计实付（含本单）达到的**最高档**。档位未配置或未达最低档
/// 都返回 None——发放路径零开销退出。
pub fn match_tier<'a>(
    tiers: &'a [DonationTier],
    cumulative_usd: f64,
) -> Option<&'a DonationTier> {
    tiers
        .iter()
        .filter(|t| t.min_usd > 0.0 && cumulative_usd + 1e-9 >= t.min_usd)
        .max_by(|a, b| {
            a.min_usd
                .partial_cmp(&b.min_usd)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// 档位发放（settle 事务内调用）。返回发放摘要（None=无档位/未置位=已发过）。
/// 第二闸：tier_granted CAS——手动补单先推进 status 的场景下，本函数在并发
/// 双入口时只有一方置位成功。
pub async fn grant_tier(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    _state: &crate::state::AppState,
    order_no: &str,
    user_id: i64,
    cumulative_usd: f64,
) -> DomainResult<Option<String>> {
    let tiers = load_tiers(&_state.repo.db).await;
    let Some(tier) = match_tier(&tiers, cumulative_usd) else {
        return Ok(None);
    };
    // CAS 置位：已发放过（重复回调/补单）rows_affected=0
    let n = sqlx::query(
        "UPDATE payment_orders SET tier_granted = TRUE \
         WHERE order_no = $1 AND NOT tier_granted",
    )
    .bind(order_no)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Ok(None);
    }
    let mut parts: Vec<String> = Vec::new();
    // ① 魔力：earn_spark_tx（行锁+幂等键双保险）
    if tier.spark > 0 {
        let idem = format!("donation_tier:{order_no}");
        crate::economy_http::earn_spark_tx(
            tx,
            user_id,
            tier.spark,
            "donation_tier",
            &idem,
        )
        .await?;
        parts.push(format!("魔力 +{}", tier.spark));
    }
    // ② 上传量：traffic_ledger + users.uploaded 双写（6h reconcile 以流水为准）
    if tier.upload_gb > 0 {
        let bytes = tier.upload_gb as i64 * 1024 * 1024 * 1024;
        sqlx::query(
            "INSERT INTO traffic_ledger (id, user_id, torrent_id, \
             delta_up, delta_down, window_start) VALUES \
             (nextval('traffic_ledger_id_seq'), $1, 0, $2, 0, now())",
        )
        .bind(user_id)
        .bind(bytes)
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        sqlx::query("UPDATE users SET uploaded = uploaded + $2 WHERE id = $1")
            .bind(user_id)
            .bind(bytes)
            .execute(&mut **tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        parts.push(format!("上传量 +{} GiB", tier.upload_gb));
    }
    // ③ 邀请码直发（shop invite 道具同款形态）
    for _ in 0..tier.invites {
        let code = crate::domain::new_invite_code();
        sqlx::query(
            "INSERT INTO invites (inviter_id, code, expires_at) \
             VALUES ($1, $2, now() + interval '72 hours')",
        )
        .bind(user_id)
        .bind(&code)
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    if tier.invites > 0 {
        parts.push(format!("邀请码 ×{}", tier.invites));
    }
    // ④ 档位勋章：已有则不重发（PK 冲突静默）
    if tier.medal_id > 0 {
        sqlx::query(
            "INSERT INTO user_medals (user_id, medal_id, source) \
             VALUES ($1, $2, 'donation') \
             ON CONFLICT (user_id, medal_id) DO NOTHING",
        )
        .bind(user_id)
        .bind(tier.medal_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        parts.push("档位勋章".into());
    }
    let summary = format!("「{}」：{}", tier.label, parts.join("、"));
    Ok(Some(summary))
}

/// 累计实付（USD）：本单入账后口径 = 已 paid 订单 amount_paid 之和。
/// 在 settle 事务内调用时本单已置 paid，直接 SUM 即含本单。
pub async fn cumulative_paid(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
) -> DomainResult<f64> {
    let v: Option<String> = sqlx::query_scalar(
        "SELECT sum(amount_paid)::text FROM payment_orders \
         WHERE user_id = $1 AND status = 'paid'",
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(v.and_then(|s| s.parse().ok()).unwrap_or(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tier(min: f64, label: &str) -> DonationTier {
        DonationTier {
            min_usd: min,
            label: label.into(),
            spark: 0,
            upload_gb: 0,
            invites: 0,
            medal_id: 0,
        }
    }

    #[test]
    fn match_tier_picks_highest_reached() {
        let tiers = vec![tier(10.0, "铜"), tier(50.0, "银"), tier(100.0, "金")];
        assert_eq!(match_tier(&tiers, 5.0).map(|t| t.label.as_str()), None);
        assert_eq!(
            match_tier(&tiers, 49.9).map(|t| t.label.as_str()),
            Some("铜")
        );
        assert_eq!(
            match_tier(&tiers, 50.0).map(|t| t.label.as_str()),
            Some("银")
        );
        assert_eq!(
            match_tier(&tiers, 999.0).map(|t| t.label.as_str()),
            Some("金")
        );
    }

    /// 浮点边界：0.1+0.2 类精度误差不应卡档（1e-9 容差）
    #[test]
    fn match_tier_float_tolerance() {
        let tiers = vec![tier(0.3, "档")];
        let cum = 0.1 + 0.2;
        assert!(match_tier(&tiers, cum).is_some());
    }

    /// min_usd<=0 的脏配置行直接被过滤（防 0 档人人触发）
    #[test]
    fn match_tier_ignores_nonpositive() {
        let tiers = vec![tier(0.0, "坏档")];
        assert!(match_tier(&tiers, 100.0).is_none());
    }
}
