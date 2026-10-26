//! 商城道具生效链（api 与 worker 共用的唯一实现）。
//!
//! 【为什么要有这个 crate】`apply_item_effect` 原先只在 api 侧
//! （`economy_http/shop_effects.rs`），于是 worker 的 `shop_fulfill`
//! 补发不了效果——崩溃后卡在冻结态的订单只能退款，用户得重买。
//! 当时的取舍是「为省一次退款把两个 crate 耦在一起不值得」，
//! 但那时还没意识到：两 crate 各写一份生效链必然漂移，而漂移的后果是
//! 「api 发了券、worker 退了款」或反过来。
//!
//! 故取第三条路：把生效链抽成本 crate，**两边引用同一份源码**。
//! 这样既不耦合（worker 不依赖 api 的 actix 栈），也不会漂移。
//!
//! 【不变的部分】生效语义与原先逐字一致：`has_effect` 的清单、
//! 幂等键口径、防重 EXISTS 判据都不动，只是搬家。

use serde_json::Value;
use sqlx::PgPool;

/// 生效错误。刻意用自有类型而非 api 的 `DomainError`：本 crate 不该
/// 知道上层错误码体系，两边各自映射即可。
#[derive(Debug, thiserror::Error)]
pub enum EffectError {
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("内部错误: {0}")]
    Internal(String),
}

pub type EffectResult = Result<(), EffectError>;

/// 生效链**真正实现了**的 kind 清单。
///
/// 下面的 match 有 `_ => {}` 兜底，意味着不在清单里的 kind 扣了钱
/// 什么也不会发生（审计 P1 命名的「花钱买空气」）。商店直接卖这类
/// SKU 是历史行为，娱乐屋把奖品绑到 SKU 时必须先过 `has_effect`。
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
            | "gacha_ticket"
    )
}

/// 「这一档发出去的是凭证（券或订单行卡），而不直接改会员状态」——凭证的价值
/// 全在后面的核销口，所以发凭证的口子必须一起问 `redeem_port_exists`（P-477）。
pub fn issues_voucher(kind: &str) -> bool {
    matches!(
        kind,
        "voucher_free"
            | "voucher_neutral"
            | "temp_invite"
            | "rename_card"
            | "makeup_card"
            | "resub_card"
    )
}

/// 发的是凭证、但全仓没有口认这张凭证 ⇒ 会员拿到就是一张废纸。
/// 商城售卖口与娱乐屋奖品绑定口都调这一句（半边闸门的教训：清单只写在
/// 售卖侧，奖品侧照样能把废纸发出去）。
pub fn voucher_without_redeem_port(kind: &str) -> bool {
    issues_voucher(kind) && !redeem_port_exists(kind)
}

/// 「花得掉」正面清单——核销口真在读这些 kind 的券或订单行（P-477）。
///
/// 放这里而不是放 api：生效链（发卡/发券那一侧）在本 crate，而核销口分成两支
/// ——`economy_http/voucher_use.rs`（券）与 `gaps_http/misc.rs`（补签按订单行张数），
/// api 侧的售卖口与娱乐屋的奖品绑定口都必须是同一把尺子：
/// - `makeup_card` / `resub_card`：补签口读 `shop_orders` ×
///   `i.kind IN ('makeup_card','resub_card')`（0066 起 resub 是同一支口的别名）；
/// - `temp_invite` / `voucher_free` / `voucher_neutral`：`voucher_use.rs`
///   认的三种券种（后两者由 `voucher_kind()` 折成 free/neutral）。
///
/// 反面 case：`rename_card` 在 `apply_item_effect` 里有臂（会发一张
/// `user_vouchers.kind='rename_card'`），但没有任何口认它——用户侧不存在自助改名
/// 端点，唯一改名口 `POST /admin/users/{id}/rename` 判 `USER_RESETPASS` 且不读券。
/// 装上核销口那天，把这一档加回来即可恢复售卖与奖品绑定。
pub fn redeem_port_exists(kind: &str) -> bool {
    matches!(
        kind,
        "makeup_card"
            | "resub_card"
            | "temp_invite"
            | "voucher_free"
            | "voucher_neutral"
    )
}

/// SKU 配置里「发放数量」的唯一读法。
pub fn spark_amount(config: &Value) -> Option<i64> {
    ["spark", "sparks", "amount"]
        .iter()
        .filter_map(|k| config.get(*k).and_then(|v| v.as_i64()))
        .find(|v| *v > 0)
}

/// 上传量卡的 GB 数。
pub fn credit_gb(config: &Value) -> Option<i64> {
    ["gb", "upload_gb", "gb_per_unit"]
        .iter()
        .filter_map(|k| config.get(*k).and_then(|v| v.as_i64()))
        .find(|v| *v > 0)
}

/// 慈善捐赠的进池金额（P-476）。
///
/// 生效链原先自写一串 `spark | sparks | price`，与上面那条「发放数量唯一读法」
/// `spark_amount`（`spark / sparks / amount`）差一个键；而 0004 起「慈善捐赠」
/// 档位的 config 一直是 `{}` ⇒ 两把尺都算不出数额，`if amount > 0` 直接静默跳过：
/// 用户被扣 1000 火花，池子 0 进账、接口回 200、没有任何失败信号
/// （10-21 现网实测 root 三单一笔都没落池）。
///
/// 收口成一处：先走 `spark_amount`，再退到档位自身的 `price`（商品配置里显式
/// 写了面值时），目录侧另有闸门 DL6 钉「在售慈善档位必须算得出 >0」。
pub fn donation_amount(config: &Value) -> i64 {
    spark_amount(config)
        .or_else(|| config.get("price").and_then(|v| v.as_i64()))
        .unwrap_or(0)
}

/// 券类 SKU 发的券种。
pub fn voucher_kind(config: &Value, sku_kind: &str) -> String {
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

/// 邀请码 / passkey 生成（crypto RNG，与 api 侧 `domain::new_passkey` 同口径）。
fn new_passkey() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rngs::OsRng;
    (0..32)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}

/// 入账（与 api 侧 `spend::earn_spark` 同口径：行锁 + 幂等键 + 台账）。
///
/// 幂等键必须在行锁**之后**检查（P0：防并发双入账）。
///
/// P-479：台账列名必须是 `spark_ledger` 真实的那套（`amount`，不是 `delta`；
/// 没有 `memo` 这一列），且 `id` 无默认值 ⇒ 必须显式 `nextval`，
/// `balance_after` 也要跟着写（否则「余额与流水对账」永远差一笔）。
/// 旧写法两条都错，于是 `gift_spark` 这一档**结构性发不出货**：语句必先撞
/// `column "delta" does not exist`（42703），整笔事务回滚——钱扣了、火花没到。
/// 顺序按 api 兄弟口：先落流水、再动余额。
pub async fn earn_spark(
    db: &PgPool,
    user_id: i64,
    amount: i64,
    kind: &str,
    idem: &str,
) -> EffectResult {
    let mut tx = db.begin().await?;
    let balance: i64 = sqlx::query_scalar(
        "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
    )
    .bind(idem)
    .fetch_one(&mut *tx)
    .await?;
    if !exists {
        let after = balance.checked_add(amount).ok_or_else(|| {
            EffectError::Internal("火花余额溢出，已拒绝入账".into())
        })?;
        sqlx::query(
            "INSERT INTO spark_ledger \
             (id, user_id, amount, kind, idempotency_key, balance_after) \
             VALUES (nextval('spark_ledger_id_seq'), $1, $2, $3, $4, $5)",
        )
        .bind(user_id)
        .bind(amount)
        .bind(kind)
        .bind(idem)
        .bind(after)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1",
        )
        .bind(user_id)
        .bind(amount)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// 授予一件 SKU 的效果。`dressup_source` 写进 `user_dressups.source`。
///
/// 商店买入与「娱乐屋奖品兑换」是两件事，都记成 `'buy'` 就等于把
/// 奖品归属抹平，故由调用方区分。
pub async fn apply_item_effect(
    db: &PgPool,
    user_id: i64,
    kind: &str,
    config: &Value,
    dressup_source: &str,
) -> EffectResult {
    match kind {
        "upload_credit" => {
            let gb = credit_gb(config).unwrap_or(0);
            sqlx::query(
                "INSERT INTO traffic_ledger \
                 (id, user_id, torrent_id, delta_up, delta_down, window_start) \
                 VALUES (nextval('traffic_ledger_id_seq'), $1, 0, $2, 0, now())",
            )
            .bind(user_id)
            .bind(gb * 1024 * 1024 * 1024)
            .execute(db)
            .await?;
            sqlx::query(
                "UPDATE users SET uploaded = uploaded + $2 WHERE id = $1",
            )
            .bind(user_id)
            .bind(gb * 1024 * 1024 * 1024)
            .execute(db)
            .await?;
        }
        "avatar_frame" | "animated_avatar" | "rainbow_id" | "rainbow_name" => {
            let item_id =
                config.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0);
            sqlx::query(
                "INSERT INTO user_dressups (user_id, item_id, source) \
                 VALUES ($1, $2, $3) ON CONFLICT (user_id, item_id) DO NOTHING",
            )
            .bind(user_id)
            .bind(item_id)
            .bind(dressup_source)
            .execute(db)
            .await?;
        }
        "invite" => {
            let code = new_passkey();
            sqlx::query(
                "INSERT INTO invites (inviter_id, code, expires_at) \
                 VALUES ($1, $2, now() + interval '72 hours')",
            )
            .bind(user_id)
            .bind(&code)
            .execute(db)
            .await?;
        }
        "voucher_free" | "voucher_neutral" => {
            let k = voucher_kind(config, kind);
            sqlx::query(
                "INSERT INTO user_vouchers (user_id, kind, source) \
                 VALUES ($1, $2, 'shop')",
            )
            .bind(user_id)
            .bind(&k)
            .execute(db)
            .await?;
        }
        // VIP：days 显式 ::int——sqlx 把 i64 绑成 bigint，PG 的
        // make_interval(days =>) 只收 int4，无隐式转换。
        // 不 cast 就是「购买 500、扣款成功效果丢失」（商城审计 P0-1）。
        "vip" | "app_vip" => {
            let days =
                config.get("days").and_then(|v| v.as_i64()).unwrap_or(30);
            sqlx::query(
                "UPDATE users SET vip_until = \
                    GREATEST(COALESCE(vip_until, now()), now()) \
                    + make_interval(days => $2::int) \
                 WHERE id = $1",
            )
            .bind(user_id)
            .bind(days)
            .execute(db)
            .await?;
        }
        "ad_free" => {
            let days =
                config.get("days").and_then(|v| v.as_i64()).unwrap_or(15);
            sqlx::query(
                "UPDATE users SET donor_until = \
                    GREATEST(COALESCE(donor_until, now()), now()) \
                    + make_interval(days => $2::int) \
                 WHERE id = $1",
            )
            .bind(user_id)
            .bind(days)
            .execute(db)
            .await?;
        }
        "custom_title" => {
            if let Some(t) = config.get("title").and_then(|v| v.as_str()) {
                if !t.trim().is_empty() && t.chars().count() <= 30 {
                    sqlx::query("UPDATE users SET title = $2 WHERE id = $1")
                        .bind(user_id)
                        .bind(t.trim())
                        .execute(db)
                        .await?;
                }
            }
            let unlock = config
                .get("unlock")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if unlock {
                // 防重只拦「仍可用」的那张（未核销且未过期）——
                // 已用过/已过期的旧券不该挡住补发。
                let usable: bool = sqlx::query_scalar(
                    "SELECT has_usable_voucher($1, 'title_unlock')",
                )
                .bind(user_id)
                .fetch_one(db)
                .await
                .unwrap_or(false);
                if !usable {
                    sqlx::query(
                        "INSERT INTO user_vouchers (user_id, kind, source) \
                         VALUES ($1, 'title_unlock', 'shop')",
                    )
                    .bind(user_id)
                    .execute(db)
                    .await?;
                }
            }
        }
        "gift_spark" => {
            let sparks = spark_amount(config).unwrap_or(0);
            if sparks > 0 {
                let item_id =
                    config.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0);
                // 幂等键优先取**购买单键**（shop:{buyer}:{key}，每次购买唯一）。
                // 旧写法只用「接收人+商品+秒级时间戳」：两个买家同一秒赠同
                // 一人会产生同键，后一笔被 earn_spark 幂等**静默吞掉**
                // （买家付了钱、接收方凭空少收——2026-10-22 审计实证）。
                // 传入 gift_idem 后同时获得「按购买单幂等」：同单重试/补发
                // 不会给接收方双倍入账。
                let idem = config
                    .get("gift_idem")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| format!("shop-gift:{s}"))
                    .unwrap_or_else(|| {
                        format!(
                            "shop-gift:{user_id}:{item_id}:{}",
                            chrono::Utc::now().timestamp()
                        )
                    });
                earn_spark(db, user_id, sparks, "shop", &idem).await?;
            }
        }
        "charity" => {
            // P-476：金额一律走 donation_amount（与 spark_amount 同一把尺）。
            let amount = donation_amount(config);
            if amount > 0 {
                let month = pool_month(chrono::Utc::now());
                sqlx::query(
                    "INSERT INTO pool_donations (user_id, amount, month) \
                     VALUES ($1, $2, $3)",
                )
                .bind(user_id)
                .bind(amount)
                .bind(&month)
                .execute(db)
                .await?;
                sqlx::query(
                    "INSERT INTO magic_pool (month, donated_total) \
                     VALUES ($1, $2) \
                     ON CONFLICT (month) DO UPDATE \
                     SET donated_total = magic_pool.donated_total + $2",
                )
                .bind(&month)
                .bind(amount)
                .execute(db)
                .await?;
            }
        }
        "rename_card" | "temp_invite" => {
            sqlx::query(
                "INSERT INTO user_vouchers (user_id, kind, source) \
                 VALUES ($1, $2, 'shop')",
            )
            .bind(user_id)
            .bind(kind)
            .execute(db)
            .await?;
        }
        // 抽卡券（0418，二轮深测 B2）：抽卡券此前只有签到 daily_free 一条
        // 获取路径（每天 1 张、攒 25 天才够一抽），商店无 SKU——gacha 对
        // 普通用户是纯橱窗。购买按 config.qty 入券账；幂等键沿用 gift_idem
        //（购买单键，多件时 shop.rs 会加 :k 后缀），重放补发不双入。
        "gacha_ticket" => {
            let n: i32 = config
                .get("qty")
                .and_then(|v| v.as_i64())
                .unwrap_or(1)
                .clamp(1, 100) as i32;
            let idem = config
                .get("gift_idem")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| format!("shop-ticket:{s}"))
                .unwrap_or_else(|| {
                    format!(
                        "shop-ticket:{user_id}:{}",
                        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
                    )
                });
            let mut tx = db.begin().await?;
            sqlx::query(
                "INSERT INTO gacha_ticket_balance (user_id, balance) \
                 VALUES ($1, 0) ON CONFLICT (user_id) DO NOTHING",
            )
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
            let bal: i32 = sqlx::query_scalar(
                "SELECT balance FROM gacha_ticket_balance \
                 WHERE user_id = $1 FOR UPDATE",
            )
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
            // 同键已入过账：幂等返回（外层 shop_orders 的 effect_applied
            // CAS 已挡住绝大多数重放，这里兜 worker 补发/崩溃重跑）
            let seen: Option<i32> = sqlx::query_scalar(
                "SELECT balance_after FROM gacha_ticket_ledger \
                 WHERE idempotency_key = $1",
            )
            .bind(&idem)
            .fetch_optional(&mut *tx)
            .await?;
            if seen.is_none() {
                let after = bal + n;
                sqlx::query(
                    "INSERT INTO gacha_ticket_ledger \
                     (user_id, delta, kind, ref_type, idempotency_key, \
                      balance_after) \
                     VALUES ($1, $2, 'shop', 'shop', $3, $4)",
                )
                .bind(user_id)
                .bind(n)
                .bind(&idem)
                .bind(after)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE gacha_ticket_balance SET balance = $2 \
                     WHERE user_id = $1",
                )
                .bind(user_id)
                .bind(after)
                .execute(&mut *tx)
                .await?;
            }
            tx.commit().await?;
        }
        _ => {} // 其余类型：权益标记后续按需扩展
    }
    Ok(())
}

/// 记账月份（与 api 侧 `economy::pool_month` 同口径）。
fn pool_month(t: chrono::DateTime<chrono::Utc>) -> String {
    use chrono::Datelike;
    format!("{:04}-{:02}", t.year(), t.month())
}

/// 自动审种·规则内核（0400）：api（裁决出口/回放）与 worker（取件评估）
/// 共用的纯函数评估器。裁决同尺纪律见模块头。
pub mod auto_review;

/// 每日火花流水结算凭证（Merkle）：worker 封存与 api 出证明共用。
pub mod merkle;

/// 金标准回归集（§18.3，≥60 例数据表）：仅测试用，不出 lib 面。
#[cfg(test)]
mod auto_review_corpus;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn effect_list_matches_kinds_with_real_sql() {
        // 清单里的每一个都必须真的有分支，否则「以为有效果」=
        // 扣了钱什么也没发生。
        for k in ["upload_credit", "invite", "vip", "gift_spark", "charity"] {
            assert!(has_effect(k), "{k} 应在生效清单内");
        }
        // 明确不实现的（历史遗留 SKU）：不在清单里，绑定时被拒。
        for k in ["no_such_kind", ""] {
            assert!(!has_effect(k), "{k} 不该在生效清单内");
        }
    }

    #[test]
    fn amount_reads_every_spelling() {
        assert_eq!(spark_amount(&json!({"spark": 1000})), Some(1000));
        assert_eq!(spark_amount(&json!({"sparks": 500})), Some(500));
        assert_eq!(spark_amount(&json!({"amount": 7})), Some(7));
        // 非正数不认：取不到正数由调用方决定拒发，不静默当 0。
        assert_eq!(spark_amount(&json!({"spark": 0})), None);
        assert_eq!(spark_amount(&json!({})), None);
    }

    #[test]
    fn redeem_port_list_matches_the_two_consumer_branches() {
        // 券侧：voucher_use.rs 认 free / neutral / temp_invite
        assert!(redeem_port_exists("voucher_free"));
        assert!(redeem_port_exists("voucher_neutral"));
        assert!(redeem_port_exists("temp_invite"));
        // 订单行侧：gaps_http/misc.rs 的补签（makeup_card 与历史别名 resub_card）
        assert!(redeem_port_exists("makeup_card"));
        assert!(redeem_port_exists("resub_card"));
        // P-477 的反面：发得出券但没人认 ⇒ 必须不在清单里
        assert!(!redeem_port_exists("rename_card"));
        assert!(!redeem_port_exists("vip")); // 状态类不走核销口，走 has_effect
                                             // 成对判据：凭证类缺核销口才算废纸
        assert!(voucher_without_redeem_port("rename_card"));
        assert!(!voucher_without_redeem_port("temp_invite"));
        assert!(!voucher_without_redeem_port("makeup_card"));
        assert!(!voucher_without_redeem_port("vip"));
        assert!(!voucher_without_redeem_port("upload_credit"));
        assert!(issues_voucher("rename_card") && !issues_voucher("vip"));
    }

    #[test]
    fn donation_amount_follows_the_canonical_reader() {
        // 与 spark_amount 同一把尺：amount 这个拼法以前在慈善分支里不认
        assert_eq!(donation_amount(&json!({"amount": 700})), 700);
        assert_eq!(donation_amount(&json!({"spark": 1000})), 1000);
        // 退路：档位写了自己的面值
        assert_eq!(donation_amount(&json!({"price": 1000})), 1000);
        // P-476 的现场：0004 起慈善档位的 config 就是 {} ⇒ 0（由目录闸门拦下）
        assert_eq!(donation_amount(&json!({})), 0);
        assert_eq!(donation_amount(&json!({"spark": 0, "price": 1000})), 1000);
    }

    #[test]
    fn credit_gb_reads_every_spelling() {
        assert_eq!(credit_gb(&json!({"gb": 10})), Some(10));
        assert_eq!(credit_gb(&json!({"upload_gb": 20})), Some(20));
        assert_eq!(credit_gb(&json!({"gb_per_unit": 5})), Some(5));
        assert_eq!(credit_gb(&json!({"gb": -1})), None);
    }

    #[test]
    fn voucher_kind_defaults_by_sku_prefix() {
        assert_eq!(voucher_kind(&json!({}), "voucher_neutral"), "neutral");
        assert_eq!(voucher_kind(&json!({}), "voucher_free"), "free");
        assert_eq!(
            voucher_kind(&json!({"kind": "neutral"}), "voucher_free"),
            "neutral"
        );
    }

    #[test]
    fn passkey_is_32_chars_lowercase_alnum() {
        let c = new_passkey();
        assert_eq!(c.len(), 32);
        assert!(c
            .chars()
            .all(|x| x.is_ascii_lowercase() || x.is_ascii_digit()));
    }

    #[test]
    fn two_passkeys_differ() {
        // 极小概率撞车，但 crypto RNG 下不该恒等。
        assert_ne!(new_passkey(), new_passkey());
    }
}
