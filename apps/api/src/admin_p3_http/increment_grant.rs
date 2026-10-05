//! 批量发放的 kind 分支实现（0286 从 increment_bulk.rs 拆出守行数门禁）。
//! 语义口径与单发路径对齐：spark 走流水、uploaded 落 traffic_ledger、
//! 券类入 user_vouchers、勋章 PK 冲突跳过、背包类 0 价单入 shop_orders。

use super::increment_bulk::IncrementBulkReq;

use super::increment_grant_item::{grant_item, grant_medal};

use crate::errors::{DomainError, DomainResult};
use crate::http::AuthUser;

pub(super) struct GrantCtx<'a> {
    pub(super) db: &'a sqlx::PgPool,
    pub(super) batch_id: &'a str,
    pub(super) amount: i64,
    pub(super) auth: &'a AuthUser,
}

/// spark：余额权威在流水。正数走 earn_spark；负数 CTE 锁行按真实差额
/// 落流水（余额截断到 0 时 |流水| < |amount|，保 sum(ledger)=balance）。
async fn grant_spark(
    ctx: &GrantCtx<'_>,
    chunk: &[i64],
) -> DomainResult<u64> {
    let db = ctx.db;
    let mut affected: u64 = 0;
    for uid in chunk {
        let idem = format!("increment_bulk:{}:{uid}", ctx.batch_id);
        if ctx.amount > 0 {
            crate::economy_http::earn_spark(
                db,
                *uid,
                ctx.amount,
                "increment_bulk",
                &idem,
            )
            .await?;
        } else {
            // 审计修复（P1 账本不变量）：旧实现按原始 amount 落流水、余额却
            // GREATEST(0,...) 截断 —— 用户余额 500 扣 1000 时流水记 -1000、
            // 余额变 0，sum(ledger) ≠ balance 从此失真。改为 CTE 里锁行取前值，
            // 流水按「前值-后值」的真实差额落（截断时 |流水| < |amount|）。
            let before_after: Option<(i64, i64)> = sqlx::query_as(
                "WITH prev AS (SELECT spark_balance AS b \
                                  FROM users WHERE id = $1 FOR UPDATE), \
                  upd AS (UPDATE users \
                          SET spark_balance = GREATEST(0, spark_balance + $2) \
                          WHERE id = $1 RETURNING spark_balance AS a) \
                 SELECT prev.b, upd.a FROM prev, upd",
            )
            .bind(uid)
            .bind(ctx.amount)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            let Some((bal_before, bal_after)) = before_after else {
                continue;
            };
            let actual_delta = bal_after - bal_before; // ≤0；截断时比 amount 接近 0
            sqlx::query(
                "INSERT INTO spark_ledger (id, user_id, amount, kind, \
                                          idempotency_key, balance_after) \
                 VALUES (nextval('spark_ledger_id_seq'), $1, $2, \
                         'increment_bulk', $3, $4)",
            )
            .bind(uid)
            .bind(actual_delta)
            .bind(&idem)
            .bind(bal_after)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        affected += 1;
    }
    Ok(affected)
}

/// uploaded：authority 在 traffic_ledger——reconcile 会把 users.uploaded
/// 重算为 sum(ledger)，裸 UPDATE 的手工加量会被对账静默清零，须落差额流水。
async fn grant_uploaded(
    ctx: &GrantCtx<'_>,
    chunk: &[i64],
) -> DomainResult<u64> {
    let bytes = ctx.amount * 1024 * 1024 * 1024;
    Ok(sqlx::query(
        "WITH targets AS ( \
            SELECT id, uploaded FROM users WHERE id = ANY($1) FOR UPDATE \
         ), upd AS ( \
            UPDATE users u SET uploaded = GREATEST(0, u.uploaded + $2) \
            FROM targets t WHERE u.id = t.id \
            RETURNING u.id, GREATEST(0, t.uploaded + $2) - t.uploaded AS delta \
         ) \
         INSERT INTO traffic_ledger (id, user_id, torrent_id, \
                                     delta_up, delta_down, window_start) \
         SELECT nextval('traffic_ledger_id_seq'), id, 0, \
                delta, 0, now() FROM upd WHERE delta <> 0",
    )
    .bind(chunk)
    .bind(bytes)
    .execute(ctx.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected())
}

/// invite：days=Some 直发 N 天临时邀请码（不走 quota_extra）；否则正数
/// 增发 quota_extra、负数回收（下限 0），与单用户 adjust 口径一致。
async fn grant_invite(
    ctx: &GrantCtx<'_>,
    chunk: &[i64],
    days: Option<i32>,
) -> DomainResult<u64> {
    let db = ctx.db;
    if let Some(days) = days {
        // 临时邀请（好学 tmp_invites 口径）：码已在手，到期自动失效
        // （status 由 worker 置 2）
        let expires = chrono::Utc::now()
            + chrono::Duration::days(days.clamp(1, 365) as i64);
        let mut affected: u64 = 0;
        for uid in chunk {
            for _ in 0..ctx.amount {
                let code = crate::domain::new_invite_code();
                sqlx::query(
                    "INSERT INTO \
                     invites (inviter_id, code, expires_at) VALUES \
                     ($1, $2, $3)",
                )
                .bind(uid)
                .bind(&code)
                .bind(expires)
                .execute(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                affected += 1;
            }
        }
        return Ok(affected);
    }
    Ok(sqlx::query(
        "UPDATE users SET quota_extra \
         = GREATEST(0, quota_extra + $2) WHERE id = ANY($1)",
    )
    .bind(chunk)
    .bind(ctx.amount)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected())
}

/// resub_card：每人 amount 张（0 价订单入包，复用 grant-item 背包语义）。
/// 字典种子 kind=makeup_card；gaps_http 补签流程引用 resub_card（历史
/// 不一致），两种都认，优先 makeup_card。
async fn grant_resub_card(
    ctx: &GrantCtx<'_>,
    chunk: &[i64],
) -> DomainResult<u64> {
    let item: Option<(i64, serde_json::Value)> = sqlx::query_as(
        "SELECT id, \
         config FROM shop_items WHERE kind IN ('makeup_card','resub_card') \
         AND active = true ORDER BY kind = 'makeup_card' DESC, id LIMIT 1",
    )
    .fetch_optional(ctx.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((item_id, config)) = item else {
        return Err(DomainError::Validation(
            "商店缺少可用的补签卡道具（makeup_card）".into(),
        ));
    };
    bulk_backpack_orders(ctx, chunk, item_id, &config).await
}

/// 背包类：逐人逐张 0 价单入 shop_orders（独立幂等键，UNIQUE 防重）。
pub(super) async fn bulk_backpack_orders(
    ctx: &GrantCtx<'_>,
    chunk: &[i64],
    item_id: i64,
    config: &serde_json::Value,
) -> DomainResult<u64> {
    let mut affected: u64 = 0;
    for uid in chunk {
        for _ in 0..ctx.amount {
            let idem = format!(
                "increment_bulk:{}:{uid}:{}",
                ctx.batch_id,
                uuid::Uuid::new_v4().simple()
            );
            sqlx::query(
                "INSERT INTO shop_orders (user_id, item_id, price, \
                                          idempotency_key, config_snapshot) \
                 VALUES ($1, $2, 0, $3, $4)",
            )
            .bind(uid)
            .bind(item_id)
            .bind(&idem)
            .bind(config)
            .execute(ctx.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            affected += 1;
        }
    }
    Ok(affected)
}

/// 总分发：increment_bulk 主流程按 kind 调用。`_auth` 仅为签名对齐预留。
#[allow(clippy::too_many_arguments)]
pub(super) async fn grant_kind(
    body: &IncrementBulkReq,
    db: &sqlx::PgPool,
    batch_id: &str,
    chunk: &[i64],
    auth: &AuthUser,
) -> DomainResult<u64> {
    let ctx = GrantCtx { db, batch_id, amount: body.amount, auth };
    match body.kind.as_str() {
        "spark" => grant_spark(&ctx, chunk).await,
        "uploaded" => grant_uploaded(&ctx, chunk).await,
        "invite" => grant_invite(&ctx, chunk, body.days).await,
        "resub_card" => grant_resub_card(&ctx, chunk).await,
        "medal" => grant_medal(
            &ctx,
            chunk,
            body.medal_id.ok_or_else(|| {
                DomainError::Validation("需选择勋章".into())
            })?,
        )
        .await,
        "item" => grant_item(
            &ctx,
            chunk,
            body.item_id.ok_or_else(|| {
                DomainError::Validation("需选择道具".into())
            })?,
        )
        .await,
        _ => unreachable!(),
    }
}
