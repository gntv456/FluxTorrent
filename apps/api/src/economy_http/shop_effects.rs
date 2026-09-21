//! 道具生效逻辑（M11）。
//! 从 economy_http.rs 按域拆出。

use super::spend::earn_spark;
use crate::economy;
use crate::errors::{DomainError, DomainResult};
use sqlx::PgPool;

pub async fn apply_item_effect(
    db: &PgPool,
    user_id: i64,
    kind: &str,
    config: &serde_json::Value,
) -> DomainResult<()> {
    match kind {
        // 上传量：等值正流量流水（§6.2 快照刷新由 worker 聚合，这里直接加账）
        "upload_credit" => {
            let gb = config.get("gb").and_then(|v| v.as_i64()).unwrap_or(0);
            sqlx::query(
                "INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, delta_down, window_start) \
                 VALUES (nextval('traffic_ledger_id_seq'), $1, 0, $2, 0, now())",
            )
            .bind(user_id)
            .bind(gb * 1024 * 1024 * 1024)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query(
                "UPDATE users SET uploaded = uploaded + $2 WHERE id = $1",
            )
            .bind(user_id)
            .bind(gb * 1024 * 1024 * 1024)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // 装扮（M25）：写入拥有记录（佩戴需显式调 /dressup/wear）
        "avatar_frame" | "animated_avatar" | "rainbow_id" | "rainbow_name" => {
            let item_id =
                config.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0);
            sqlx::query(
                "INSERT INTO user_dressups (user_id, item_id, \
                 source) VALUES ($1, $2, 'buy') ON CONFLICT (user_id, item_id) \
                 DO NOTHING",
            )
            .bind(user_id)
            .bind(item_id)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // 邀请：直接发一枚 72h 有效邀请码
        "invite" => {
            let code = crate::domain::new_invite_code();
            sqlx::query(
                "INSERT INTO invites (inviter_id, code, \
             expires_at) VALUES ($1, $2, now() + interval '72 hours')",
            )
            .bind(user_id)
            .bind(&code)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // 免费券/中性券（0073，Gazelle FL token 口径）：买入库为库存，使用走 /me/vouchers/use
        "voucher_free" | "voucher_neutral" => {
            let kind = config
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("free");
            sqlx::query(
                "INSERT INTO user_vouchers (user_id, kind, \
                 source) VALUES ($1, $2, 'shop')",
            )
            .bind(user_id)
            .bind(kind)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // VIP 待遇到期延展（0079 G18：购买日 ≥ 到期日则从今天起算，否则续期——断购不惩罚）
        "vip" | "app_vip" => {
            let days =
                config.get("days").and_then(|v| v.as_i64()).unwrap_or(30);
            sqlx::query(
                "UPDATE users SET \
                    vip_until = GREATEST(COALESCE(vip_until, now()), now()) + make_interval(days => $2), \
                    donor = TRUE \
                 WHERE id = $1",
            )
            .bind(user_id)
            .bind(days)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // 免广告（donor 待遇；NP 口径：15 天档）
        "ad_free" => {
            let days =
                config.get("days").and_then(|v| v.as_i64()).unwrap_or(15);
            sqlx::query(
                "UPDATE users SET donor_until = GREATEST(COALESCE(donor_until, now()), now()) + make_interval(days => $2) \
                 WHERE id = $1",
            )
            .bind(user_id)
            .bind(days)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // 自定义头衔（NP 5000 魔力口径）：config.title 由商店 SKU 预置；用户可后续在 UserCP 改（同价）
        "custom_title" => {
            if let Some(t) = config.get("title").and_then(|v| v.as_str()) {
                if !t.trim().is_empty() && t.chars().count() <= 30 {
                    sqlx::query("UPDATE users SET title = $2 WHERE id = $1")
                        .bind(user_id)
                        .bind(t.trim())
                        .execute(db)
                        .await
                        .map_err(|e| DomainError::Internal(e.into()))?;
                }
            }
        }
        // 审计修复（P1 花钱买空气）：下列 SKU 此前落入 `_ => {}` 兜底，扣款后无任何效果。
        // gift_spark：等值火花立即入账（earn 幂等键绑订单号语义 shop:{uid}:{item}）
        "gift_spark" => {
            let sparks = config
                .get("spark")
                .or_else(|| config.get("sparks"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            if sparks > 0 {
                let item_id =
                    config.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0);
                let idem = format!(
                    "shop-gift:{user_id}:{item_id}:{}",
                    chrono::Utc::now().timestamp()
                );
                earn_spark(db, user_id, sparks, "shop", &idem).await?;
            }
        }
        // charity：捐赠入 magic_pool + pool_donations（去向可查，v_pool_honor 可见）。
        // 金额取 config.spark，缺省按 SKU 价格全额入池（购买即捐赠语义）。
        "charity" => {
            let amount = config
                .get("spark")
                .or_else(|| config.get("sparks"))
                .and_then(|v| v.as_i64())
                .or_else(|| config.get("price").and_then(|v| v.as_i64()))
                .unwrap_or(0);
            if amount > 0 {
                let month = economy::pool_month(chrono::Utc::now());
                sqlx::query(
                    "INSERT INTO pool_donations (user_id, \
                     amount, month) VALUES ($1, $2, $3)",
                )
                .bind(user_id)
                .bind(amount)
                .bind(&month)
                .execute(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                sqlx::query(
                    "INSERT INTO magic_pool (month, donated_total) VALUES ($1, $2) \
                     ON CONFLICT (month) DO UPDATE SET donated_total = magic_pool.donated_total + $2",
                )
                .bind(&month)
                .bind(amount)
                .execute(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            }
        }
        // rename_card：入库存（生效走 UserCP 改名消费，见 username_change_logs）
        "rename_card" | "temp_invite" => {
            sqlx::query(
                "INSERT INTO user_vouchers (user_id, kind, \
                 source) VALUES ($1, $2, 'shop')",
            )
            .bind(user_id)
            .bind(kind)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        _ => {} // 其余类型：权益标记后续按需扩展（佩戴/生效周期）
    }
    Ok(())
}
