//! 促销到期/魔力池/保种退出结算。
//! 从 jobs.rs 按域拆出。

use chrono::Datelike;
use sqlx::PgPool;

/// 促销到期回收（M06：到期自动回收，无残留）。
/// 审计修复：保留一年历史窗口——hr_enforce 建 H&R 快照时要按 completed_at 时点
/// 回查「当时是否处于免费窗口」豁免，物理删掉近期促销会让回查失明（误判违规）。
/// 计费查询全部走 `starts_at <= now() < ends_at` 生效窗口，不受历史保留影响。
pub async fn expire_promotions(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        "DELETE FROM promotions WHERE ends_at <= now() - interval '365 days'",
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// 魔法池双免联动（原实现为死功能：promo_started 全库无写入点）。
/// 站点时区 UTC+8 每月 1-3 号检查上月是否达标（donated_total >= goal），
/// 达标则开全站 scope 双免促销（x2free，3 天），幂等靠 promo_started 标记。
pub async fn magic_pool_promo(db: &PgPool) -> anyhow::Result<u64> {
    let now_site = chrono::Utc::now() + chrono::Duration::hours(8);
    if !(1..=3).contains(&now_site.day()) {
        return Ok(0);
    }
    let res = sqlx::query(
        r#"
        WITH prev AS (
            SELECT to_char(date_trunc('month', $1::date) - interval '1 month', 'YYYY-MM') AS month
        ),
        pool AS (
            SELECT mp.month, mp.promo_started
            FROM magic_pool mp, prev
            WHERE mp.month = prev.month AND mp.donated_total >= mp.goal AND NOT mp.promo_started
        ),
        started AS (
            UPDATE magic_pool mp SET promo_started = TRUE
            FROM pool WHERE mp.month = pool.month
            RETURNING mp.month
        )
        INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source)
        SELECT 'global', NULL, 'x2free', now(), now() + interval '3 days', 'magic_pool'
        FROM started
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(now_site)
    .execute(db)
    .await?;
    let n = res.rows_affected();
    if n > 0 {
        tracing::info!(n, "magic_pool promo started (x2free, 3d)");
    }
    Ok(n)
}
