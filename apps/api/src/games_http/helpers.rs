//! 游戏域共享助手：经济读数/赔率常量/每小时限次/幂等键。
//! 从 games_http.rs 按域拆出。

use actix_web::web;
use serde::Deserialize;
use uuid::Uuid;

use crate::errors::{DomainError, DomainResult};
use crate::games::{self, MAX_PLAYS_PER_HOUR};
use crate::state::AppState;

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
pub(super) async fn grant_item(
    db: &sqlx::PgPool,
    user_id: i64,
    item_key: &str,
    qty: i32,
    game: &str,
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
        "SELECT unlimited, stock, per_user FROM arcade_items WHERE key = $1 AND enabled FOR SHARE",
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
            "SELECT COALESCE(SUM(qty)::bigint, 0) FROM arcade_item_grants WHERE item_key = $1",
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
        "SELECT COALESCE(SUM(qty)::bigint,0) FROM arcade_item_grants WHERE item_key = $1 AND user_id = $2",
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
        "INSERT INTO arcade_item_grants (item_key, user_id, qty, game, side, idem)          VALUES ($1, $2, $3, $4, 'rand', $5)",
    )
    .bind(item_key)
    .bind(user_id)
    .bind(qty)
    .bind(game)
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
pub(super) async fn load_pool(db: &sqlx::PgPool, game: &str) -> Result<Pool, DomainError> {
    let rows: Vec<(i64, String, i32, i64, String, Option<String>, i32, Option<i64>)> =
        sqlx::query_as(
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
            games::PoolEntry { label, weight: u32::try_from(weight.max(0)).unwrap_or(u32::MAX), kind: k }
        })
        .collect();
    games::validate_pool(&entries, ticket).map_err(|e| DomainError::Validation(e.to_string()))?;
    Ok(Pool { ticket, entries })
}

/// 读取游戏经济设置键（0109 参数化；缺省回落代码默认值 T3）
pub(super) async fn eco_i64(
    state: &web::Data<std::sync::Arc<AppState>>,
    key: &str,
    default: i64,
) -> i64 {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         $1)::bigint, $2)",
    )
    .bind(key)
    .bind(default)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(default)
}

/// 浮点设置键（赔率类支持小数，如猜大小 1.9x）
pub(super) async fn eco_f64(
    state: &web::Data<std::sync::Arc<AppState>>,
    key: &str,
    default: f64,
) -> f64 {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         $1)::double precision, $2)",
    )
    .bind(key)
    .bind(default)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(default)
}

/// 刮刮乐档位（五档可配；10x 留空/合计不为 100 时按余数推导，缺省 45/30/15/8/2）
pub(super) async fn scratch_odds(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> Result<games::ScratchOdds, DomainError> {
    games::ScratchOdds::try_from_parts(
        eco_i64(state, "games_scratch_empty_pct", 45).await,
        eco_i64(state, "games_scratch_half_pct", 30).await,
        eco_i64(state, "games_scratch_one_pct", 15).await,
        eco_i64(state, "games_scratch_two_pct", 8).await,
        eco_i64(state, "games_scratch_ten_pct", 2).await,
    )
    .map_err(DomainError::Validation)
}

/// 农场作物有效期（天；0 = 永不枯萎）
pub(super) async fn farm_wither_days(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> i64 {
    eco_i64(state, "farm_wither_days", 5).await.clamp(0, 60)
}

/// 农场市场价刷新窗口（小时）。四审 L8：0109 登记了该键但无人读，站长改数字
/// 不生效；此处接线后窗口仍钳 1..24，保证它是 3600 的整数倍且不超过一天。
pub(super) async fn farm_market_hours(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> i64 {
    eco_i64(state, "farm_market_window_hours", 4)
        .await
        .clamp(1, 24)
}

/// 猜大小赔率（千分比）。倍数设置键缺省 1.9 —— **必须 < 2.0**：
/// 2.0 时 EV 恰为 1.0（不回收）且可双向零风险对冲，见 games.rs 常量说明。
/// 上限钳到 1999‰（运行时 EV 防线，P2）：设置键是管理员可写参数，此前上限 10_000‰
/// 意味着误配/越权写入 ≥2000‰ 即把游戏变成增发开关。
pub(super) async fn bigsmall_mult_permille(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> i64 {
    let mult = eco_f64(state, "games_bigsmall_win_mult", 1.9).await;
    ((mult * 1000.0).round() as i64).clamp(0, 1_999)
}

/// 限流作用域：即时赌局与农场**分开计数**。
/// 农场是慢玩法，一次种满 6 块地不该吃掉 6 次即时下注额度（旧实现共用 `rl:games`）。
#[derive(Clone, Copy)]
pub(super) enum RateScope {
    Instant,
    Farm,
}

impl RateScope {
    fn redis_key(&self, user_id: i64) -> String {
        match self {
            RateScope::Instant => format!("rl:games:{user_id}"),
            RateScope::Farm => format!("rl:farm:{user_id}"),
        }
    }
    fn setting(&self) -> (&'static str, i64) {
        match self {
            RateScope::Instant => {
                ("games_max_plays_per_hour", MAX_PLAYS_PER_HOUR)
            }
            RateScope::Farm => ("farm_max_plays_per_hour", 30),
        }
    }
}

/// 每小时限次（Redis INCR + EXPIRE；上限走设置键）。
/// 审计修复（P2-7）：Redis 不可用时**拒绝**（fail-close）——旧实现 `unwrap_or(0)` 会让限流静默失效。
pub(super) async fn check_rate_scoped(
    state: &web::Data<std::sync::Arc<AppState>>,
    redis: &redis::aio::ConnectionManager,
    user_id: i64,
    scope: RateScope,
) -> DomainResult<()> {
    use redis::AsyncCommands;
    let (setting, default) = scope.setting();
    let limit = eco_i64(state, setting, default).await;
    let key = scope.redis_key(user_id);
    let mut conn = redis.clone();
    let n: i64 = conn.incr(&key, 1).await.map_err(|_| {
        DomainError::Internal(anyhow::anyhow!("限流服务不可用"))
    })?;
    if n == 1 {
        let _: () = conn.expire(&key, 3600).await.map_err(|_| {
            DomainError::Internal(anyhow::anyhow!("限流服务不可用"))
        })?;
    }
    if n > limit {
        return Err(DomainError::RateLimited);
    }
    Ok(())
}

pub(super) async fn check_rate(
    state: &web::Data<std::sync::Arc<AppState>>,
    redis: &redis::aio::ConnectionManager,
    user_id: i64,
) -> DomainResult<()> {
    check_rate_scoped(state, redis, user_id, RateScope::Instant).await
}

/// 读取限流器当前已用次数（只读，不 +1；供前端显示「剩余 N 局」）。
/// 取不到（Redis 未计数/异常）返回 None，调用方自行回落。
pub(super) async fn limit_used(
    state: &web::Data<std::sync::Arc<AppState>>,
    user_id: i64,
    farm: bool,
) -> Option<i64> {
    use redis::AsyncCommands;
    let key = if farm {
        RateScope::Farm.redis_key(user_id)
    } else {
        RateScope::Instant.redis_key(user_id)
    };
    let mut conn = state.redis.clone();
    conn.get::<_, Option<i64>>(&key).await.ok().flatten()
}

/// 下注校验（上限走设置键 games_max_bet）
pub(super) async fn check_bet(
    state: &web::Data<std::sync::Arc<AppState>>,
    bet: i64,
) -> Result<(), String> {
    let max = eco_i64(state, "games_max_bet", games::MAX_BET).await;
    if bet <= 0 {
        return Err("下注必须为正数".into());
    }
    if bet > max {
        return Err(format!("单次下注不能超过 {max} 魔力"));
    }
    Ok(())
}

#[derive(Deserialize)]
pub(super) struct BetReq {
    pub(super) bet: i64,
    /// 客户端幂等键（审计 P2-8）：网络重试/双击须带同一键，缺省回落随机键保持兼容
    #[serde(default)]
    pub(super) idempotency_key: Option<String>,
}

pub(super) fn idem_key(
    prefix: &str,
    user_id: i64,
    client: &Option<String>,
) -> String {
    match client {
        Some(k) if !k.trim().is_empty() && k.len() <= 128 => {
            format!("game-{prefix}:{user_id}:{k}")
        }
        _ => format!("game-{prefix}:{}:{}", user_id, Uuid::new_v4()),
    }
}
