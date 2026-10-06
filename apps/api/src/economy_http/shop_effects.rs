//! 道具生效逻辑（M11）。
//! 从 economy_http.rs 按域拆出。

use super::spend::earn_spark;
use crate::economy;
use crate::errors::{DomainError, DomainResult};
use sqlx::PgPool;

/// 生效链**真正实现了**的 kind 清单。下面的 match 加 `_ => {}` 兜底，意味着
/// 不在这张清单里的 kind 扣了钱什么也不会发生（审计 P1 命名的「花钱买空气」）。
/// 商店直接卖这类 SKU 是历史行为，娱乐屋把奖品绑到 SKU 时必须先过这道判定。
/// 新增生效分支时，记得同时把它加进清单——漏加只会让绑定被拒，不会静默。
pub fn has_effect(kind: &str) -> bool {
    matches!(
        kind,
        "upload_credit"
            | "avatar_frame"
            | "animated_avatar"
            | "rainbow_id"
            | "rainbow_name"
            | "invite"
            | "voucher_free"
            | "voucher_neutral"
            | "vip"
            | "app_vip"
            | "ad_free"
            | "custom_title"
            | "gift_spark"
            | "charity"
            | "rename_card"
            | "temp_invite"
    )
}

/// SKU 配置里「发放数量」的唯一读法。
///
/// 此前商店购买读 `spark`、管理端发放读 `amount`，两把尺各量各的：站里那张
/// `{"spark":1000}` 的魔力卡从后台发出去回 200 却零入账（实测 A1）。
/// 口径以购买路径（线上真正在用的那份）为主，其余写法一并兼容；
/// **取不到正数一律返回 None，由调用方决定拒发**，不再各自 `unwrap_or(0)` 空转。
pub fn spark_amount(config: &serde_json::Value) -> Option<i64> {
    ["spark", "sparks", "amount"]
        .iter()
        .filter_map(|k| config.get(*k).and_then(|v| v.as_i64()))
        .find(|v| *v > 0)
}

/// 上传量卡的 GB 数，同上口径。
pub fn credit_gb(config: &serde_json::Value) -> Option<i64> {
    ["gb", "upload_gb", "gb_per_unit"]
        .iter()
        .filter_map(|k| config.get(*k).and_then(|v| v.as_i64()))
        .find(|v| *v > 0)
}

/// 券类 SKU 发的券种。`free` / `neutral` 是 user_vouchers.kind 的取值域，
/// 与 SKU 的 kind 同名但不等价，缺省按 SKU 前缀推。
pub fn voucher_kind(config: &serde_json::Value, sku_kind: &str) -> String {
    config
        .get("kind")
        .and_then(|v| v.as_str())
        .filter(|s| matches!(*s, "free" | "neutral"))
        .unwrap_or(if sku_kind == "voucher_neutral" {
            "neutral"
        } else {
            "free"
        })
        .to_string()
}

/// 授予一件 SKU 的效果。`dressup_source` 写进 `user_dressups.source`：
/// 商店买入与「娱乐屋奖品兑换」是两件事，都记成 `'buy'` 就等于把奖品归属抹平。
pub async fn apply_item_effect(
    db: &PgPool,
    user_id: i64,
    kind: &str,
    config: &serde_json::Value,
    dressup_source: &str,
) -> DomainResult<()> {
    match kind {
        // 上传量：等值正流量流水（§6.2 快照刷新由 worker 聚合，这里直接加账）
        "upload_credit" => {
            let gb = credit_gb(config).unwrap_or(0);
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
        // 装扮（M25）：写入拥有记录（佩戴需显式调 /dressup/wear）。
        // 0207：item_id 直接用 SKU 自身 id（apply 的调用方把 config.item_id
        // 覆写为 body.item_id，种子 SKU 无该键时据此 fallback）。
        "avatar_frame" | "animated_avatar" | "rainbow_id" | "rainbow_name" => {
            let item_id =
                config.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0);
            sqlx::query(
                "INSERT INTO user_dressups (user_id, item_id, \
                 source) VALUES ($1, $2, $3) ON CONFLICT (user_id, item_id) \
                 DO NOTHING",
            )
            .bind(user_id)
            .bind(item_id)
            .bind(dressup_source)
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
            // days 显式 ::int：sqlx 把 i64 绑成 bigint，而 PG 的
            // make_interval(days =>) 只收 int4，bigint 无隐式转换——
            // 不 cast 就是「购买 500、扣款成功效果丢失」（商城审计 P0-1）
            sqlx::query(
                "UPDATE users SET \
                    vip_until = GREATEST(COALESCE(vip_until, now()), now()) + make_interval(days => $2::int), \
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
                "UPDATE users SET donor_until = GREATEST(COALESCE(donor_until, now()), now()) + make_interval(days => $2::int) \
                 WHERE id = $1",
            )
            .bind(user_id)
            .bind(days)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // 自定义头衔（NP 5000 魔力口径，0292 起双语义）：
        //  * config.title 预置（后台结构化表单可填）：购买即直设该头衔；
        //  * config.unlock=true（商店种子缺省形态）：购买解锁「UserCP 自助
        //    改头衔」资格——每人想要的文字不同，预置 title 只会让 8 万买空。
        //    消费端见 /me/profile 的 title 编辑门槛（has_title_unlock）。
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
            let unlock = config
                .get("unlock")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if unlock {
                // user_vouchers 无 (user_id,kind) 唯一约束（券类靠重复购买堆行），
                // 解锁券防重：先查后插，重复购买/效果重放只留一张
                let owned: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM user_vouchers \
                     WHERE user_id = $1 AND kind = 'title_unlock')",
                )
                .bind(user_id)
                .fetch_one(db)
                .await
                .unwrap_or(false);
                if !owned {
                    sqlx::query(
                        "INSERT INTO user_vouchers (user_id, kind, source) \
                         VALUES ($1, 'title_unlock', 'shop')",
                    )
                    .bind(user_id)
                    .execute(db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                }
            }
        }
        // 审计修复（P1 花钱买空气）：下列 SKU 此前落入 `_ => {}` 兜底，扣款后无任何效果。
        // gift_spark：等值火花立即入账（earn 幂等键绑订单号语义 shop:{uid}:{item}）
        "gift_spark" => {
            let sparks = spark_amount(config).unwrap_or(0);
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
