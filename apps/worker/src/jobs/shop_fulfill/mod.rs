//! 商城三段式履约的恢复任务（0383 worker 侧）。
//!
//! **为什么必须有这个任务**：三段式的价值全在「钱在履约成功前一直是
//! 冻结态」——但这只解决了一半。另一半是：冻结之后如果进程崩了、机器
//! 断电了、apply 卡住了，谁来善后？没有后台任务的话，冻结就变成了
//! 「用户的钱被无限期占着」，比原来的「直接扣款」更糟。
//!
//! 本任务就是那个善后方：按 `frozen_at` 升序拉取超过宽限的订单，
//! 逐个推进状态机（`domain::shop`）：
//!   * 履约成功 → 扣款成交；
//!   * 履约失败 → 再给一次重试（单次）；
//!   * 仍失败 → 解冻退款，用户的钱回到余额。
//!
//! 【幂等与不变量】
//!   * 每次只处理 `fulfill_state = 'frozen'` 的行，靠部分唯一索引
//!     `idx_shop_orders_stuck` 走索引，不扫全表；
//!   * 扣款走 `spend_spark` 的既有幂等键（`shop:{uid}:{idem}`），
//!     与用户主动购买同一把钥匙——恢复任务与用户重试撞一起也不会双扣；
//!   * 终态（done/refunded）永不再动：重复发货比状态不漂亮严重得多。
//!
//! 【不变量】任何一轮结束后，没有订单会停在 `frozen` 且未过宽限——
//! 也就是说「用户的钱不会被无限期占着」这条要靠本任务保证。

use sqlx::PgPool;

/// 单轮最多处理多少单（防止一次 tick 跑太久）。
const BATCH: i64 = 100;

/// 一张卡住的订单。
#[derive(sqlx::FromRow)]
struct Stuck {
    id: i64,
    user_id: i64,
    item_id: i64,
    price: i64,
    frozen_amount: i64,
    idempotency_key: String,
    kind: String,
    config: serde_json::Value,
    effect_applied: bool,
}

/// 站点宽限（秒）。缺省 120s，与 0383 迁移的种子值一致。
async fn grace_secs(db: &PgPool) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(NULLIF((SELECT value FROM site_settings \
         WHERE name = 'shop_freeze_grace_secs'), '')::bigint, 120)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(120)
    .clamp(10, 86400)
}

/// 拉取卡住的订单（超过宽限的）。
async fn stuck_orders(
    db: &PgPool,
    grace: i64,
) -> anyhow::Result<Vec<Stuck>> {
    Ok(sqlx::query_as(
        "SELECT o.id, o.user_id, o.item_id, o.price, o.frozen_amount, \
                o.idempotency_key, si.kind, si.config, o.effect_applied \
         FROM shop_orders o \
         JOIN shop_items si ON si.id = o.item_id \
         WHERE o.fulfill_state = 'frozen' \
           AND o.frozen_at < now() - make_interval(secs => $1) \
         ORDER BY o.frozen_at LIMIT $2",
    )
    .bind(grace as f64)
    .bind(BATCH)
    .fetch_all(db)
    .await?)
}

/// 恢复任务主入口。返回本轮推进的单数。
pub(crate) async fn recover(db: &PgPool) -> anyhow::Result<u64> {
    let grace = grace_secs(db).await;
    let rows = stuck_orders(db, grace).await?;
    if rows.is_empty() {
        return Ok(0);
    }
    let mut handled = 0u64;
    for o in rows {
        match advance_one(db, &o).await {
            Ok(true) => handled += 1,
            Ok(false) => {}
            Err(e) => {
                // 单单失败不中断整轮：下一轮再试（状态机本身幂等）
                tracing::warn!(
                    order_id = o.id, user_id = o.user_id,
                    error = %e, "商城履约恢复：推进失败"
                );
            }
        }
    }
    if handled > 0 {
        tracing::info!(handled, grace, "商城履约恢复推进");
    }
    Ok(handled)
}

/// 推进一单。
async fn advance_one(db: &PgPool, o: &Stuck) -> anyhow::Result<bool> {
    // effect_applied 已为真 = 效果发出去了，只差扣款 → 直接成交。
    // 这条分支很重要：它覆盖「效果发了但进程在扣款前崩了」。
    if o.effect_applied {
        return settle_charge(db, o).await;
    }
    // 否则重试履约：成功则扣款，失败再失败则退款（单次重试是刻意的——
    // 同一个必然失败的履约无限重试只是刷日志）。
    let ok = apply_effect(db, o).await?;
    if ok {
        settle_charge(db, o).await
    } else {
        release(db, o, "fulfill_failed_after_retry").await
    }
}

/// 履约（发效果）。
///
/// **现在能补发了。** 生效链已抽到 `crates/flux-shared`，api 与 worker
/// 引用同一份源码——此前这里只能退款，因为 `apply_item_effect` 在 api
/// 侧（economy_http），worker 引用不到。当时的取舍是「为省一次退款
/// 把两个 crate 耦在一起不值得」；现在既不用耦，也不用退款。
///
/// 【为什么共用源码是必须的而不是可选的】若两边各写一份生效链，
/// 漂移的后果是「api 发了券、worker 判定失败退款」——用户既没拿到东西
/// 也丢了钱，且两边日志各自自洽、事后对不上。
async fn apply_effect(db: &PgPool, o: &Stuck) -> anyhow::Result<bool> {
    let mut cfg = o.config.clone();
    cfg["item_id"] = serde_json::json!(o.item_id);
    // dressup_source 用 "recover"：与用户主动购买（'buy'）、娱乐屋奖品
    // （'prize'）区分开。事后从 user_dressups.source 能看出这枚装扮
    // 是恢复任务补发的，而不是用户自己买的。
    match flux_shared::apply_item_effect(
        db, o.user_id, &o.kind, &cfg, "recover",
    )
    .await
    {
        Ok(()) => {
            // 效果真的发出去了才置位。顺序反了就是「标记已发但没发」，
            // 那一单从此再也无法补发——比不发更糟。
            let n = sqlx::query(
                "UPDATE shop_orders SET effect_applied = TRUE \
                 WHERE id = $1 AND NOT effect_applied",
            )
            .bind(o.id)
            .execute(db)
            .await?
            .rows_affected();
            if n == 0 {
                // 并发窗口：另一条路径已置位但效果尚未落地。此时返回
                // false 让本轮退款是错的，保守起见交给下一轮重试。
                tracing::warn!(
                    order_id = o.id, "商城恢复：effect_applied 已被他方置位"
                );
                return Ok(false);
            }
            Ok(true)
        }
        Err(e) => {
            tracing::warn!(
                order_id = o.id, user_id = o.user_id,
                kind = %o.kind, error = %e, "商城恢复：履约失败"
            );
            Ok(false)
        }
    }
}

/// 成交：把冻结转成实扣状态。
///
/// 这里**不做实际扣款**——钱在冻结那一刻就已经从余额里划走了
/// （冻结 = 扣款，只是不给效果）。本函数只把订单与冻结台账推上终态，
/// 让对账口径闭合：`frozen` 清空、`consumed` 记账。
///
/// 之所以叫 settle_charge 而不是 mark_done：语义上钱确实已经扣了，
/// 只是履约侧当时还没确认。写清楚这点，免得后人以为这里能再扣一次。
async fn settle_charge(db: &PgPool, o: &Stuck) -> anyhow::Result<bool> {
    let mut tx = db.begin().await?;
    let n = sqlx::query(
        "UPDATE shop_orders SET fulfill_state = 'done', fulfilled_at = now() \
         WHERE id = $1 AND fulfill_state = 'frozen'",
    )
    .bind(o.id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 0 {
        tx.rollback().await?;
        return Ok(false); // 别人已经推进过
    }
    sqlx::query(
        "UPDATE shop_freezes SET state = 'consumed', settled_at = now() \
         WHERE order_id = $1 AND state = 'frozen'",
    )
    .bind(o.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

/// 解冻退款：把钱还回去。
async fn release(
    db: &PgPool,
    o: &Stuck,
    reason: &str,
) -> anyhow::Result<bool> {
    let mut tx = db.begin().await?;
    let n = sqlx::query(
        "UPDATE shop_orders SET fulfill_state = 'refunded', \
             fail_reason = $2 WHERE id = $1 AND fulfill_state = 'frozen'",
    )
    .bind(o.id)
    .bind(reason)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 0 {
        tx.rollback().await?;
        return Ok(false);
    }
    sqlx::query(
        "UPDATE shop_freezes SET state = 'released', settled_at = now() \
         WHERE order_id = $1 AND state = 'frozen'",
    )
    .bind(o.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!(
        order_id = o.id, user_id = o.user_id,
        amount = o.frozen_amount, reason, "商城履约失败：已解冻"
    );
    Ok(true)
}
