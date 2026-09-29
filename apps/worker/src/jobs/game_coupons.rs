//! 口粮券发放（优先级① 核心行为联动轻量版）。
//!
//! 判定「每日做种满 N 小时」直接复用 `seeding_reward` 流水：其幂等键形如
//! `seeding:{user}:{yyyymmddhh}`，当日不同小时段条数 = 当日做种小时数。
//! 这样**不新增任何做种时长累计表、零风险不改资金管线**；发放幂等靠
//! `food_coupon_grants(user_id, day)` 主键，job 重跑 / 跨日补跑安全。
//!
//! 阈值走设置键 `games_coupon_seed_hours`（缺省 6），运维可热调，无需改代码重发。

use sqlx::PgPool;

/// 每小时跑一次（与 `seeding_reward` 同节奏，挂在 run.rs 的 hour_tick）。
/// 当日已发过卡的用户被主键挡掉，所以重复执行零副作用。
pub async fn grant_food_coupons(db: &PgPool) -> anyhow::Result<u64> {
    // 1) 算「当日做种小时 ≥ 阈值」且「今日尚未发卡」的用户，写入幂等表并返回其 id
    let new_grantees: Vec<(i64,)> = sqlx::query_as(
        r#"
        WITH cfg AS (
            SELECT COALESCE((
                SELECT value::bigint FROM site_settings
                WHERE name = 'games_coupon_seed_hours'
            ), 6) AS thr
        ),
        today AS (
            SELECT date_trunc('day', now() AT TIME ZONE 'UTC' + interval '8 hours') AS start,
                   (now() AT TIME ZONE 'UTC' + interval '8 hours')::date AS d
        ),
        seed_hours AS (
            SELECT l.user_id,
                   count(DISTINCT split_part(l.idempotency_key, ':', 3)) AS hrs
            FROM spark_ledger l, today t
            WHERE l.kind = 'seeding_reward'
              AND l.created_at >= t.start
              AND l.created_at <  t.start + interval '1 day'
            GROUP BY l.user_id
            HAVING count(DISTINCT split_part(l.idempotency_key, ':', 3))
                   >= (SELECT thr FROM cfg)
        )
        INSERT INTO food_coupon_grants (user_id, day)
        SELECT s.user_id, t.d
        FROM seed_hours s, today t
        WHERE NOT EXISTS (
            SELECT 1 FROM food_coupon_grants g
            WHERE g.user_id = s.user_id AND g.day = t.d
        )
        ON CONFLICT (user_id, day) DO NOTHING
        RETURNING user_id
        "#,
    )
    .fetch_all(db)
    .await?;

    let ids: Vec<i64> = new_grantees.into_iter().map(|r| r.0).collect();
    if ids.is_empty() {
        return Ok(0);
    }

    // 2) 给达标用户 +1 口粮券（与写入幂等表分两步：INSERT...RETURNING 已天然排除重复，
    //    再批量 UPDATE 余额，避免同事务内对刚插入行的二次竞争）。
    let n = sqlx::query(
        "UPDATE users SET food_coupons = food_coupons + 1 WHERE id = ANY($1)",
    )
    .bind(&ids)
    .execute(db)
    .await?
    .rows_affected();
    Ok(n)
}
