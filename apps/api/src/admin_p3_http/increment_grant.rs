//! 批量发放的 kind 分支实现（0286 从 increment_bulk.rs 拆出）。
//! 语义口径与单发路径对齐：spark 走流水、uploaded 落 traffic_ledger、
//! 券类入 user_vouchers、勋章 PK 冲突跳过、背包类 0 价单入 shop_orders。
//!
//! 0291 起每批自带事务：库存占位与发放结果同生同死。此前是「先 +qty 占位、
//! 失败再手工 -qty」，进程或后续语句一崩，那一格配额就永久消失了（少卖一格
//! 且无人知情）。事务里越界直接报错，回滚由数据库负责。

use super::increment_bulk::IncrementBulkReq;
use super::increment_grant_item::{grant_item, grant_medal};
use super::increment_stock::{resub_item_id, stock_take};

use crate::errors::{DomainError, DomainResult};
use crate::http::AuthUser;

pub(super) type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

pub(super) fn dberr(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

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
    tx: &mut Tx<'_>,
    chunk: &[i64],
) -> DomainResult<u64> {
    let mut affected: u64 = 0;
    for uid in chunk {
        let idem = format!("increment_bulk:{}:{uid}", ctx.batch_id);
        if ctx.amount > 0 {
            crate::economy_http::expect_spent(
                crate::economy_http::earn_spark_tx(
                    tx,
                    *uid,
                    ctx.amount,
                    "increment_bulk",
                    &idem,
                )
                .await?,
            )?;
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
            .fetch_optional(&mut **tx)
            .await
            .map_err(dberr)?;
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
            .execute(&mut **tx)
            .await
            .map_err(dberr)?;
        }
        affected += 1;
    }
    Ok(affected)
}

/// uploaded：authority 在 traffic_ledger——reconcile 会把 users.uploaded
/// 重算为 sum(ledger)，裸 UPDATE 的手工加量会被对账静默清零，须落差额流水。
///
/// 0291：流水必须带 reason + operator_id，并把 torrent_id 写成 NULL 而不是 0。
/// 此前这一分支只写数量，事后完全读不出「这 5TB 是谁在哪一批补的」——
/// 同一条流水表上单发路径与商店购买都带来源，唯独批量发放不带。
async fn grant_uploaded(
    ctx: &GrantCtx<'_>,
    tx: &mut Tx<'_>,
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
                                     delta_up, delta_down, window_start, \
                                     reason, operator_id) \
         SELECT nextval('traffic_ledger_id_seq'), id, NULL, \
                delta, 0, now(), $3, $4 FROM upd WHERE delta <> 0",
    )
    .bind(chunk)
    .bind(bytes)
    .bind(format!(
        "bulk:{} {:+}GB",
        &ctx.batch_id[..8.min(ctx.batch_id.len())],
        ctx.amount
    ))
    .bind(ctx.auth.id)
    .execute(&mut **tx)
    .await
    .map_err(dberr)?
    .rows_affected())
}

/// invite：days=Some 直发 N 天临时邀请码（不走 quota_extra）；否则正数
/// 增发 quota_extra、负数回收（下限 0），与单用户 adjust 口径一致。
async fn grant_invite(
    ctx: &GrantCtx<'_>,
    tx: &mut Tx<'_>,
    chunk: &[i64],
    days: Option<i32>,
) -> DomainResult<u64> {
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
                    "INSERT INTO invites (inviter_id, code, expires_at) \
                     VALUES ($1, $2, $3)",
                )
                .bind(uid)
                .bind(&code)
                .bind(expires)
                .execute(&mut **tx)
                .await
                .map_err(dberr)?;
                affected += 1;
            }
        }
        return Ok(affected);
    }
    Ok(sqlx::query(
        "UPDATE users SET quota_extra = GREATEST(0, quota_extra + $2) \
         WHERE id = ANY($1)",
    )
    .bind(chunk)
    .bind(ctx.amount)
    .execute(&mut **tx)
    .await
    .map_err(dberr)?
    .rows_affected())
}

/// resub_card：每人 amount 张（0 价订单入包，复用 grant-item 背包语义）。
async fn grant_resub_card(
    ctx: &GrantCtx<'_>,
    tx: &mut Tx<'_>,
    chunk: &[i64],
) -> DomainResult<u64> {
    let item_id = resub_item_id(ctx.db).await?;
    let config: serde_json::Value =
        sqlx::query_scalar("SELECT config FROM shop_items WHERE id = $1")
            .bind(item_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(dberr)?
            .unwrap_or_else(|| serde_json::json!({ "stackable": true }));
    stock_take(tx, item_id, ctx.amount * chunk.len() as i64).await?;
    bulk_backpack_orders(ctx, tx, chunk, item_id, &config).await
}

/// 背包类：逐人逐张 0 价单入 shop_orders（独立幂等键，UNIQUE 防重）。
pub(super) async fn bulk_backpack_orders(
    ctx: &GrantCtx<'_>,
    tx: &mut Tx<'_>,
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
            .execute(&mut **tx)
            .await
            .map_err(dberr)?;
            affected += 1;
        }
    }
    Ok(affected)
}

/// 总分发：一批一个事务。失败时这一批整体回滚，前面几批已真实发出
/// （由调用方把批次状态回写成 partial，界面上看得见发到哪一步）。
#[allow(clippy::too_many_arguments)]
pub(super) async fn grant_kind(
    body: &IncrementBulkReq,
    db: &sqlx::PgPool,
    batch_id: &str,
    chunk: &[i64],
    auth: &AuthUser,
) -> DomainResult<u64> {
    let ctx = GrantCtx {
        db,
        batch_id,
        amount: body.amount,
        auth,
    };
    let mut tx = db.begin().await.map_err(dberr)?;
    let n = dispatch(&ctx, &mut tx, body, chunk).await?;
    tx.commit().await.map_err(dberr)?;
    Ok(n)
}

async fn dispatch(
    ctx: &GrantCtx<'_>,
    tx: &mut Tx<'_>,
    body: &IncrementBulkReq,
    chunk: &[i64],
) -> DomainResult<u64> {
    match body.kind.as_str() {
        "spark" => grant_spark(ctx, tx, chunk).await,
        "uploaded" => grant_uploaded(ctx, tx, chunk).await,
        "invite" => grant_invite(ctx, tx, chunk, body.days).await,
        "resub_card" => grant_resub_card(ctx, tx, chunk).await,
        "medal" => {
            if let Some(d) = body.medal_days {
                if !(1..=3650).contains(&d) {
                    return Err(DomainError::Validation(
                        "medal_days 需在 1-3650 之间".into(),
                    ));
                }
            }
            grant_medal(
                ctx,
                tx,
                chunk,
                body.medal_id.ok_or_else(|| {
                    DomainError::Validation("需选择勋章".into())
                })?,
                body.medal_days,
            )
            .await
        }
        "item" => {
            grant_item(
                ctx,
                tx,
                chunk,
                body.item_id.ok_or_else(|| {
                    DomainError::Validation("需选择道具".into())
                })?,
            )
            .await
        }
        _ => Err(DomainError::Validation("kind 不合法".into())),
    }
}
