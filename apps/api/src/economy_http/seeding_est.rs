//! 做种收益预估与官种统计（只读）。
//! 从 economy_http.rs 按域拆出。

use sqlx::PgPool;

/// 官种做种数与体积（只读展示，不参与结算）。
/// 让用户直接看到「我保了多少官种」，配合 0488 官种倍率形成激励闭环。
pub async fn official_seeding(db: &PgPool, user_id: i64) -> (i64, i64) {
    sqlx::query_as(
        "SELECT count(*)::bigint, COALESCE(sum(t.size), 0)::bigint \
         FROM snatches sn JOIN torrents t ON t.id = sn.torrent_id \
         WHERE sn.user_id = $1 AND sn.seeding AND t.official_tag",
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .unwrap_or((0, 0))
}

/// 做种收益计数拆解：总数里哪几颗真的在计收益、哪几颗没有、为什么。
///
/// 分类必须与结算的过滤顺序一致且互斥（先端口、再回连、最后新鲜度），
/// 否则「总数 ≠ 计收益 + 各原因之和」，用户第一眼就会发现对不上。
/// `suspended` 是账号级停发：为真时所有种子都不计，与逐颗原因无关。
pub struct EarningSplit {
    pub earning: i64,
    pub port_closed: i64,
    pub unreachable: i64,
    pub stale: i64,
    pub suspended: bool,
}

pub async fn earning_split(
    db: &PgPool,
    user_id: i64,
    stale_secs: i64,
) -> EarningSplit {
    let row: Option<(i64, i64, i64, i64, Option<bool>)> =
        sqlx::query_as(SPLIT_SQL)
            .bind(user_id)
            .bind(stale_secs)
            .fetch_optional(db)
            .await
            .ok()
            .flatten();
    let (earning, port_closed, unreachable, stale, suspended) =
        row.unwrap_or((0, 0, 0, 0, None));
    EarningSplit {
        earning,
        port_closed,
        unreachable,
        stale,
        suspended: suspended.unwrap_or(false),
    }
}

const SPLIT_SQL: &str = r#"
        SELECT
            count(*) FILTER (WHERE cat = 'earning') AS earning,
            count(*) FILTER (WHERE cat = 'port') AS port_closed,
            count(*) FILTER (WHERE cat = 'unreach') AS unreachable,
            count(*) FILTER (WHERE cat = 'stale') AS stale,
            (SELECT (u.status >= 2
                     OR COALESCE(u.cheat_banned_until > now(), false)
                     OR EXISTS (
                         SELECT 1 FROM cheat_events c
                         WHERE c.user_id = u.id
                           AND c.resolved_at IS NULL
                           AND NOT c.false_positive
                           AND (c.agent LIKE 'ghost:%'
                                OR c.agent LIKE 'speed:%'
                                OR c.agent LIKE 'reset:%'
                                OR c.agent LIKE 'corr:%')
                     ))
             FROM users u WHERE u.id = $1) AS suspended
        FROM (
            SELECT CASE
                WHEN s.last_port IS NULL OR s.last_port <= 0 THEN 'port'
                WHEN COALESCE(s.connectable = 0, false) THEN 'unreach'
                WHEN s.last_seen_at IS NULL
                  OR s.last_seen_at
                     <= now() - ($2::bigint * interval '1 second')
                  THEN 'stale'
                ELSE 'earning'
            END AS cat
            FROM snatches s
            WHERE s.user_id = $1 AND s.seeding
        ) x
        "#;

/// 预估本用户下一个小时的做种收益。
///
/// 口径必须与 `apps/worker/src/jobs/seeding.rs` 的 per_torrent 段逐条对齐：
/// 同一组 DB 函数，同一套过滤（幽灵行 / 僵尸 / 停权 / 作弊未处置），
/// 同一组乘数（盒子倍率 / 官种倍率 / 发布者衰减）。任一侧漂移都会让
/// 「我的魔力」页面的预估与实际到账不符 —— 改这里必须同步改那边。
///
/// 历史坑（0488 核对时发现）：旧预估只做了「种子加成 × 官种倍率」，
/// 漏掉盒子倍率与发布者衰减 ⇒ 盒子用户显示偏低、自发布者显示偏高，
/// 用户一对比流水就认为站点算错。现改为结算语句的逐条镜像。
pub async fn hourly_estimate(
    db: &PgPool,
    user_id: i64,
    stale_secs: i64,
) -> i64 {
    sqlx::query_scalar(EST_SQL)
        .bind(user_id)
        .bind(stale_secs)
        .fetch_one(db)
        .await
        .unwrap_or(0)
}

const EST_SQL: &str = r#"
        WITH p AS MATERIALIZED (
            SELECT *, seeding_official_mult() AS official_mult
            FROM seeding_params()
        ),
        per_torrent AS (
            SELECT seeding_torrent_bonus(
                t.size, t.seeders,
                (EXTRACT(EPOCH FROM (now() - t.created_at))
                 / 86400.0)::double precision,
                t.times_completed,
                (GREATEST(s.seeded_seconds, 0) / 3600.0)::double precision,
                p.vol_base, p.rarity_k, p.rarity_exp, p.scale)
                * COALESCE(sb.bonus_factor, 1.0)
                * (CASE WHEN t.official_tag
                        THEN p.official_mult ELSE 1.0 END)
                * seeding_publisher_mult(
                      t.owner_id = u.id,
                      (EXTRACT(EPOCH FROM (now() - t.created_at))
                       / 86400.0)::double precision)
                AS b,
                donor_privileged(u.id) AS donor
            FROM users u
            JOIN snatches s ON s.user_id = u.id AND s.seeding
            LEFT JOIN seedbox_effective sb ON sb.user_id = u.id
            JOIN torrents t ON t.id = s.torrent_id
              AND s.last_port > 0
              AND NOT COALESCE(s.connectable = 0, false)
            CROSS JOIN p
            WHERE u.id = $1
              AND u.status < 2
              AND NOT COALESCE(u.cheat_banned_until > now(), false)
              AND s.last_seen_at > now()
                  - ($2::bigint * interval '1 second')
              AND NOT EXISTS (
                  SELECT 1 FROM cheat_events c
                  WHERE c.user_id = u.id AND c.resolved_at IS NULL
                    AND NOT c.false_positive
                    AND (c.agent LIKE 'ghost:%'
                         OR c.agent LIKE 'speed:%'
                         OR c.agent LIKE 'reset:%'
                         OR c.agent LIKE 'corr:%')
                  LIMIT 1
              )
        ),
        s2 AS (
            SELECT count(*) AS n,
                   COALESCE(sum(b), 0) AS bonus_raw,
                   COALESCE(bool_or(donor), FALSE) AS donor
            FROM per_torrent
        )
        -- 没有任何一颗种子通过结算过滤时，worker 侧根本不会为该用户开
        -- 流水（per_user 取不到行），预估必须同步回 0。若直接算
        -- seeding_hourly(0, ...) 会得到 base（默认 10），页面显示「每小时
        -- +10」而实际一分不到账 —— 这正是核对要堵的口子。
        SELECT CASE WHEN n = 0 THEN 0::bigint
                    ELSE seeding_hourly(bonus_raw, p.base, p.cap, p.curve_k,
                                        p.donor_mult, donor) END
        FROM s2 CROSS JOIN p
        "#;
