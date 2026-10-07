//! 下载计费与站点统计（M02/M05）。
//! 从 torrents.rs 按域拆出。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 付费下载扣费（0086，NP 价格 口径）：下载前调用。
/// 免费/发布者本人/已购 → 直接放行；否则原子扣款 → 发布者得 (100-税)% → 税入当月魔法池。
/// 全程单事务，余额不足返回校验错误。
/// 审计修复（P0 错账）：旧版直接 UPDATE users.spark_balance、不写 spark_ledger ——
/// 买方扣款与发布者入账都被每小时「余额=流水重算」回滚（下载变免费/收益被抹除），
/// 且 /admin/spark-logs 完全看不到这类变动。改为流水驱动（幂等键绑定 torrent+user）。
pub async fn charge_for_download(
    db: &PgPool,
    user_id: i64,
    torrent_id: i64,
) -> DomainResult<()> {
    let row: Option<(i64, Option<i64>)> = sqlx::query_as(
        "SELECT price, \
         owner_id FROM torrents WHERE id = $1 AND approval_status = 1",
    )
    .bind(torrent_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((price, owner_id)) = row else {
        return Err(DomainError::NotFound(torrent_id));
    };
    if price <= 0 || owner_id == Some(user_id) {
        return Ok(());
    }
    let purchased: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrent_purchases WHERE user_id = \
         $1 AND torrent_id = $2)",
    )
    .bind(user_id)
    .bind(torrent_id)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if purchased {
        return Ok(());
    }
    let tax: i32 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::int FROM site_settings WHERE \
         name = 'upload_price_tax'), 30)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(30)
    .clamp(0, 90);
    let net = price * (100 - tax as i64) / 100;
    let tax_amount = price - net;
    // 三轮遗留：月度计费帽与绩效结算同一月键口径（站点时区 UTC+8）
    let month = (chrono::Utc::now() + chrono::Duration::hours(8))
        .format("%Y-%m")
        .to_string();

    let mut tx = db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 余额校验（锁行）：不足直接拦下，不发流水
    let balance: i64 = sqlx::query_scalar(
        "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if balance < price {
        return Err(DomainError::Validation(format!(
            "魔力不足：该种子为付费种子（{price} 魔力），请先充值或签到攒魔力"
        )));
    }
    // 买方扣款流水（幂等键含 torrent：同一种子只扣一次，重放安全）
    sqlx::query(
        "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \
         VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'torrent_buy', 'torrent', $3, $4, $5)",
    )
    .bind(user_id)
    .bind(-price)
    .bind(torrent_id)
    .bind(format!("torrent-buy:{user_id}:{torrent_id}"))
    .bind(balance - price)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 余额快照同事务扣减（P1）：只写流水不扣快照，余额校验读到的一直是旧值，
    // 用户可在小时级重算前的窗口内连续超花，重算后快照变负。
    sqlx::query(
        "UPDATE users SET spark_balance = spark_balance - $2 WHERE id = $1",
    )
    .bind(user_id)
    .bind(price)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 发布者入账流水（owner_id 经 torrents FK 保证非空语义；净得 = 价 - 税）
    if let Some(owner) = owner_id {
        let obal: i64 = sqlx::query_scalar(
            "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
        )
        .bind(owner)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .unwrap_or(0);
        sqlx::query(
            "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \
             VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'torrent_sell', 'torrent', $3, $4, $5)",
        )
        .bind(owner)
        .bind(net)
        .bind(torrent_id)
        .bind(format!("torrent-sell:{user_id}:{torrent_id}"))
        .bind(obal + net)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        sqlx::query(
            "UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1",
        )
        .bind(owner)
        .bind(net)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    if tax_amount > 0 {
        sqlx::query(
            "INSERT INTO magic_pool (month, donated_total) VALUES ($1, $2) \
             ON CONFLICT (month) DO UPDATE SET donated_total = magic_pool.donated_total + $2",
        )
        .bind(&month)
        .bind(tax_amount)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    sqlx::query(
        "INSERT INTO torrent_purchases (user_id, torrent_id, price) \
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(torrent_id)
    .bind(price)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 站点统计（M10 概览，旧站首页口径）
pub async fn site_stats(db: &PgPool) -> DomainResult<serde_json::Value> {
    let users: i64 =
        sqlx::query_scalar("SELECT count(*) FROM users WHERE status < 2")
            .fetch_one(db)
            .await
            .unwrap_or(0);
    let torrents: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents WHERE approval_status = 1",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let dead: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents WHERE approval_status = 1 AND \
         seeders = 0",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    // sum(bigint) 在 PG 返回 numeric —— sqlx 按 i64 解码必失败，曾配合
    // unwrap_or(0) 把「做种总量」静默归零（首页恒 0.0B）。::bigint 显式收口。
    let seed_size: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(t.size), 0)::bigint FROM torrents t \
         JOIN (SELECT DISTINCT torrent_id FROM snatches WHERE seeding) s ON s.torrent_id = t.id",
    )
    .fetch_one(db)
    .await
    .unwrap_or_else(|e| {
        // 统计类查询失败不 500，但必须留痕——静默 0 会让「没数据」与「查挂了」不可分
        tracing::warn!(%e, "site_stats seed_size 查询失败");
        0
    });
    Ok(serde_json::json!({
        "users": users, "torrents": torrents, "dead": dead, "seed_size": seed_size
    }))
}
