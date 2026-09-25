//! 认证字幕人身份（0149）：自动授予复扫（worker）+ 后台手动授予/撤销。
//! 展示字段（cert_tier）由列表/主页查询 LEFT JOIN 吐出。

use sqlx::PgPool;

/// 自动授予复扫（worker 每小时；幂等：PK + source=auto 才自动撤）：
///   certified —— 三阈值同时满足（产量 / 均分 / 票数；0 = 不限该条）
///   gold      —— subtitle_awards 获奖 ≥ subcert_min_awards
/// 掉标自动撤（仅 source=auto 行）；admin 手动行不自动动。
pub(crate) async fn subcert_sweep(db: &PgPool) -> anyhow::Result<(u64, u64)> {
    let mut cfgs: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for (k, v) in sqlx::query_as::<_, (String, String)>(
        "SELECT name, value FROM site_settings WHERE name LIKE 'subcert_%'",
    )
    .fetch_all(db)
    .await?
    {
        cfgs.insert(k, v);
    }
    let val = |k: &str, d: f64| -> f64 {
        cfgs.get(k).and_then(|v| v.trim().parse().ok()).unwrap_or(d)
    };
    let min_subs = val("subcert_min_subtitles", 10.0) as i64;
    let min_rating = val("subcert_min_rating", 8.0);
    let min_votes = val("subcert_min_votes", 5.0) as i64;
    let min_awards = val("subcert_min_awards", 1.0) as i64;

    let mut granted = 0u64;
    let mut revoked = 0u64;

    // gold 档：获奖次数达标（0 = 关闭整档：不授也不撤）
    if min_awards > 0 {
        let n = sqlx::query(
            r#"
            INSERT INTO user_subtitle_certs (user_id, tier, source, reason)
            SELECT a.user_id, 'gold', 'auto',
                   '金字字幕获奖 ' || count(*) || ' 次'
            FROM subtitle_awards a
            WHERE a.rank > 0 AND a.user_id IS NOT NULL
            GROUP BY a.user_id
            HAVING count(*) >= $1
            ON CONFLICT (user_id, tier) DO UPDATE
              SET revoked_at = NULL
              WHERE user_subtitle_certs.revoked_at IS NOT NULL
            "#,
        )
        .bind(min_awards)
        .execute(db)
        .await?
        .rows_affected();
        granted += n;
        // 掉标撤（获奖数不足，且行是 auto 来源）
        let n = sqlx::query(
            r#"
            UPDATE user_subtitle_certs c SET revoked_at = now()
            WHERE c.tier = 'gold' AND c.revoked_at IS NULL
              AND c.source = 'auto'
              AND (SELECT count(*) FROM subtitle_awards a
                   WHERE a.user_id = c.user_id AND a.rank > 0) < $1
            "#,
        )
        .bind(min_awards)
        .execute(db)
        .await?
        .rows_affected();
        revoked += n;
    }

    // certified 档：三阈值同时满足（某项 0 = 跳过该条件）
    if min_subs > 0 || min_rating > 0.0 || min_votes > 0 {
        let n = sqlx::query(
            r#"
            INSERT INTO user_subtitle_certs (user_id, tier, source, reason)
            SELECT s.user_id, 'certified', 'auto',
                   '字幕 ' || count(*) || ' 条 · 均分 ' ||
                   round(avg(s.avg_score)::numeric, 1)
            FROM (
                SELECT s2.user_id,
                       (s2.rating_sum::float8
                        / NULLIF(s2.rating_count, 0)) AS avg_score
                FROM subtitles s2
                WHERE s2.deleted_at IS NULL AND s2.status = 1
                  AND NOT s2.anon
                  AND ($1 <= 0 OR s2.rating_count >= $1)
                  AND ($2 <= 0 OR s2.rating_sum::float8
                       / NULLIF(s2.rating_count, 0) >= $2)
            ) s
            JOIN users u ON u.id = s.user_id AND u.status < 2
            GROUP BY s.user_id
            HAVING count(*) >= $3
            ON CONFLICT (user_id, tier) DO UPDATE
              SET revoked_at = NULL
              WHERE user_subtitle_certs.revoked_at IS NOT NULL
            "#,
        )
        .bind(min_votes)
        .bind(min_rating)
        .bind(min_subs)
        .execute(db)
        .await?
        .rows_affected();
        granted += n;
        // 掉标撤（任一条件不再满足；仅 auto 行）
        let n = sqlx::query(
            r#"
            UPDATE user_subtitle_certs c SET revoked_at = now()
            WHERE c.tier = 'certified' AND c.revoked_at IS NULL
              AND c.source = 'auto'
              AND c.user_id NOT IN (
                SELECT s.user_id FROM (
                    SELECT s2.user_id,
                           (s2.rating_sum::float8
                            / NULLIF(s2.rating_count, 0)) AS avg_score
                    FROM subtitles s2
                    WHERE s2.deleted_at IS NULL AND s2.status = 1
                      AND NOT s2.anon
                      AND ($1 <= 0 OR s2.rating_count >= $1)
                      AND ($2 <= 0 OR s2.rating_sum::float8
                           / NULLIF(s2.rating_count, 0) >= $2)
                ) s
                GROUP BY s.user_id HAVING count(*) >= $3
              )
            "#,
        )
        .bind(min_votes)
        .bind(min_rating)
        .bind(min_subs)
        .execute(db)
        .await?
        .rows_affected();
        revoked += n;
    }

    if granted + revoked > 0 {
        tracing::info!(granted, revoked, "subtitle certs swept");
    }
    Ok((granted, revoked))
}
