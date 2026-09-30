//! 娱乐玩法奖池与物品发放（DB 侧）：加载池并关闸校验、按目录发物品。
//!
//! 从 helpers.rs 拆出：helpers 管「设置读数 + 限次 + 幂等」这类横切件，
//! 池与发放是另一类职责，合在一起会撞 300 行上限并让两类改动互相纠缠。

use sqlx::PgPool;

use crate::errors::DomainError;
use crate::games;

/// 发放结果。回落不是错误 —— 它是「物品发不出去，按回落价折魔力」这条**已被 EV 计入**
/// 的路径，所以必须把原因带回公示页，不能让玩家以为拿到了物品。
pub(super) enum GrantOutcome {
    Granted,
    FellBack(&'static str),
}

/// sqlx 错误只实现了 From<anyhow::Error>，显式转一层，不让它冒到 `?` 上
pub(super) fn dberr(e: sqlx::Error) -> DomainError {
    DomainError::Internal(anyhow::Error::from(e))
}

/// 发一件物品：物品存在性、全服库存、每人上限、幂等**在同一个事务里**判。
/// 分开判会留 TOCTOU —— 并发双抽能同时通过「还有余量」的检查。
/// 限量物品的余量由发放账反推，不另存一份「已用数」（第二份清单必然漂移）。
///
/// `side` 决定这一笔进哪一侧的账：`rand` 由奖池 EV 闸管，`det`（周常/赛季）
/// 绕得过 EV 闸，只能被发放预算看见 —— 分不开这两类，确定侧就是一条没人拦的门。
pub(super) async fn grant_item(
    db: &PgPool,
    user_id: i64,
    item_key: &str,
    qty: i32,
    game: &str,
    side: &str,
    idem: &str,
) -> Result<GrantOutcome, DomainError> {
    let mut tx = db.begin().await.map_err(dberr)?;
    let seen: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM arcade_item_grants WHERE idem = $1)",
    )
    .bind(idem)
    .fetch_one(&mut *tx)
    .await
    .map_err(dberr)?;
    if seen {
        tx.commit().await.map_err(dberr)?;
        return Ok(GrantOutcome::Granted);
    }
    let item: Option<(bool, i64, i32)> = sqlx::query_as(
        r#"
        SELECT unlimited, stock, per_user
        FROM arcade_items
        WHERE key = $1 AND enabled FOR SHARE
    "#,
    )
    .bind(item_key)
    .fetch_optional(&mut *tx)
    .await
    .map_err(dberr)?;
    let (unlimited, stock, per_user) = match item {
        Some(v) => v,
        None => {
            tx.rollback().await.map_err(dberr)?;
            return Ok(GrantOutcome::FellBack("物品不存在或已停用"));
        }
    };
    if !unlimited {
        let used: i64 = sqlx::query_scalar(
            r#"
        SELECT COALESCE(SUM(qty)::bigint, 0)
        FROM arcade_item_grants
        WHERE item_key = $1
    "#,
        )
        .bind(item_key)
        .fetch_one(&mut *tx)
        .await
        .map_err(dberr)?;
        if stock - used < i64::from(qty) {
            tx.rollback().await.map_err(dberr)?;
            return Ok(GrantOutcome::FellBack("全服库存耗尽"));
        }
    }
    let mine: i64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(qty)::bigint,0)
        FROM arcade_item_grants
        WHERE item_key = $1 AND user_id = $2
    "#,
    )
    .bind(item_key)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(dberr)?;
    if mine + i64::from(qty) > i64::from(per_user) {
        tx.rollback().await.map_err(dberr)?;
        return Ok(GrantOutcome::FellBack("已达每人上限"));
    }
    sqlx::query(
        r#"
        INSERT INTO arcade_item_grants
            (item_key, user_id, qty, game, side, idem)
        VALUES ($1, $2, $3, $4, $5, $6)
    "#,
    )
    .bind(item_key)
    .bind(user_id)
    .bind(qty)
    .bind(game)
    .bind(side)
    .bind(idem)
    .execute(&mut *tx)
    .await
    .map_err(dberr)?;
    tx.commit().await.map_err(dberr)?;
    Ok(GrantOutcome::Granted)
}

/// 奖池：票价 + 档位。两者同源一行，避免「票价在表里、档位在表里、却各读一处」。
pub(super) struct Pool {
    pub ticket: i64,
    pub entries: Vec<games::PoolEntry>,
}

/// 从 arcade_pools / arcade_pool_entries 加载奖池并**关闸校验**。
/// 物品位的价值 JOIN 自 arcade_items.anchor —— 权威折算价，不是登记价；
/// 物品停用/缺目录项时 anchor 为 NULL，会被 validate_pool 当空头承诺拒掉。
/// 不合法直接 Err（拒绝服务）：不猜旧值、不回落缺省。
pub(super) async fn load_pool(
    db: &PgPool,
    game: &str,
) -> Result<Pool, DomainError> {
    let rows: Vec<(
        i64,
        String,
        i32,
        i64,
        String,
        Option<String>,
        i32,
        Option<i64>,
    )> = sqlx::query_as(
        r#"
            SELECT p.ticket, e.label, e.weight, e.payout,
                   e.kind, e.item_key, e.qty, i.anchor
              FROM arcade_pools p
              JOIN arcade_pool_entries e ON e.pool_key = p.key
         LEFT JOIN arcade_items i        ON i.key = e.item_key AND i.enabled
             WHERE p.game = $1 AND p.enabled AND e.enabled
          ORDER BY p.sort, e.sort
            "#,
    )
    .bind(game)
    .fetch_all(db)
    .await
    .map_err(dberr)?;
    let ticket = rows.first().map(|r| r.0).unwrap_or(0);
    let entries: Vec<games::PoolEntry> = rows
        .into_iter()
        .map(|(_, label, weight, payout, kind, item_key, qty, anchor)| {
            let k = if kind == "item" {
                games::EntryKind::Item {
                    item_key: item_key.unwrap_or_default(),
                    qty: qty.max(1),
                    anchor: anchor.unwrap_or(0),
                }
            } else {
                games::EntryKind::Magic { multiples: payout }
            };
            games::PoolEntry {
                label,
                weight: u32::try_from(weight.max(0)).unwrap_or(u32::MAX),
                kind: k,
            }
        })
        .collect();
    games::validate_pool(&entries, ticket)
        .map_err(|e| DomainError::Validation(e.to_string()))?;
    Ok(Pool { ticket, entries })
}
