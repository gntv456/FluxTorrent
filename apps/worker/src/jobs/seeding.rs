//! 做种激励。
//! 从 jobs.rs 按域拆出。

use sqlx::PgPool;

/// 做种收益小时结算（0074 改造：稀有度衰减 + 时长饱和，NP/U3D 口径融合）。
/// 每用户每小时一条流水，幂等键 = seeding:{user}:{yyyymmddhh}。
///
/// 做种收益结算（每小时一次，全站批量）。
///
/// 公式（**公式本体在 DB 函数里**，见迁移 0133，worker/API 共用同一真相源）：
///   每颗种子 s_i = ( 种子档位分 + 个人做种时长档位分 )
///                × min(1, log10(1+size_GB)/log10(1+vol_base))   -- 体积对数饱和
///                × (1 + rarity_k × seeders^-rarity_exp)         -- 稀有度加成
///                × scale                                        -- 总标定
///   时魔 = ( base + floor( (2/π)·cap·atan(Σ × curve_k) ) ) × (donor ? donor_mult : 1)
///
/// 档位分：
///   种子维度（第一命中优先）—— 濒危 2.0（seeders≤1 且 完成≥3）/ 高龄 1.5（>365天）/ 老种 1.0（>180天）
///                            / 大体积 0.75（≥100GiB）/ 中体积 0.5（≥25GiB）/ 日常 0.25
///   个人做种时长 —— ≥1年 +2.0 / 6-12月 +1.0 / 3-6月 +0.75 / 1-3月 +0.5 / 不足1月 0
///
/// 改造要点（2026-09-19，对照 NP/Gazelle/U3D 后）：① 补体积因子堵「堆小种」套利
/// （旧公式 1000 颗 1MB 能拿 ~205/h，比保 6 颗濒危大种还高）；② 个人做种时长从
/// 「衰减乘数」改为「加分档位」（三家都奖励长期保种，我们此前方向相反）；
/// ③ 稀有度从「只惩罚热门」改为「加成独苗」（seeders 大时仍趋近 1）。
/// 参数全部走 site_settings（`seeding_*`），标签/曲线可热调。标定见迁移注释。
///
/// 反假保种（0071）：回连不可达且从无上传的做种行不计。
/// 反假保种（0071）：回连不可达且从无上传的做种行不计。
/// 数据新鲜度：`last_seen_at` 超过僵尸阈值（max(2h, 2×announce_interval)）的行不计 ——
/// 客户端崩溃/卸载不会发 stopped 事件，`sweep_stale_peers` 虽会清理标记，但那是另一个 tick
/// 的任务；结算不该依赖"另一个 job 恰好跑过"，所以这里独立过滤一次（同一阈值函数）。
pub async fn seeding_reward(
    db: &PgPool,
    stale_secs: i64,
) -> anyhow::Result<u64> {
    let hour = chrono::Utc::now().format("%Y%m%d%H").to_string();
    let res = sqlx::query(
        r#"
        INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
        SELECT nextval('spark_ledger_id_seq'), d.id, d.amount, 'seeding_reward', d.idem
        FROM (
            -- MATERIALIZED：参数行必须只求值一次。若不显式物化，优化器可能把
            -- seeding_params() 内联进 nestloop 内层 → 每颗种子读 8 次 site_settings。
            WITH p AS MATERIALIZED (SELECT * FROM seeding_params()),
            per_torrent AS (
                -- donor_privileged（0295）：真捐赠永久，或 vip_until / donor_until
                -- 任一未过期。此前这里直接读 u.donor，而魔力购买 VIP 会顺带把
                -- donor 置 TRUE 且全库无复位路径 ⇒ 30 天待遇变永久。
                -- P0-2（2026-10-07 保种组审计）：幽灵做种过滤从「connectable=0 且
                -- uploaded=0」升级为三条件——port=0（未开监听，纯 curl 伪造）或
                -- 回连不可达的行一律不计收益。实测旧口径下 port=0 的裸 HTTP
                -- GET 即可令 seeding=true 领收益，且 uploaded>0/connectable 历史值
                -- 都能绕过旧过滤。
                -- cheat 拉黑（P1 累进处置）：存在任一未处置的 ghost/speed/reset
                -- 类作弊事件的用户整体停发（管理组处置后自动恢复）。
                SELECT u.id AS user_id, donor_privileged(u.id) AS donor,
                       seeding_torrent_bonus(
                           t.size,
                           t.seeders,
                           (EXTRACT(EPOCH FROM (now() - t.created_at)) / 86400.0)::double precision,
                           t.times_completed,
                           (GREATEST(s.seeded_seconds, 0) / 3600.0)::double precision,
                           p.vol_base, p.rarity_k, p.rarity_exp, p.scale
                       ) AS b
                FROM users u
                JOIN snatches s ON s.user_id = u.id AND s.seeding
                JOIN torrents t ON t.id = s.torrent_id
                  AND s.last_port > 0
                  AND NOT COALESCE(s.connectable = 0, false)
                CROSS JOIN p
                WHERE u.status < 2
                  AND s.last_seen_at > now() - ($2::bigint * interval '1 second')
                  AND NOT EXISTS (
                      SELECT 1 FROM cheat_events c
                      WHERE c.user_id = u.id AND c.resolved_at IS NULL
                        AND (c.agent LIKE 'ghost:%' OR c.agent LIKE 'speed:%'
                             OR c.agent LIKE 'reset:%'
                             OR c.agent = 'connectable')
                      LIMIT 1
                  )
            ),
            per_user AS (
                SELECT user_id, donor, sum(b) AS bonus_raw
                FROM per_torrent GROUP BY user_id, donor
            )
            SELECT pu.user_id AS id,
                   seeding_hourly(pu.bonus_raw, p.base, p.cap, p.curve_k, p.donor_mult, pu.donor) AS amount,
                   'seeding:' || pu.user_id || ':' || $1 AS idem
            FROM per_user pu CROSS JOIN p
        ) d
        WHERE NOT EXISTS (
            SELECT 1 FROM spark_ledger l WHERE l.idempotency_key = d.idem
        )
        "#,
    )
    .bind(&hour)
    .bind(stale_secs)
    .execute(db)
    .await?;
    // 0072：顺手物化做种体积（ptppUserInfo 的 seedingSize 字段来源；earners 已扫全量做种行）
    sqlx::query(
        "UPDATE users u SET seeding_size = COALESCE(s.sz, 0) \
         FROM (SELECT s2.user_id, sum(t2.size) AS sz FROM snatches s2 \
               JOIN torrents t2 ON t2.id = s2.torrent_id WHERE s2.seeding GROUP BY s2.user_id) s \
         WHERE u.id = s.user_id",
    )
    .execute(db)
    .await?;
    // 刷新余额快照（权威在流水，快照仅展示）。
    // 行锁串行化：与 API 的「锁行读余额→插流水→改余额」事务互斥，避免聚合快照
    // 覆写并发事务刚落账的余额（丢失更新）；单事务内先锁后算，聚合与更新一致。
    // 审计修复（P0 锁表）：旧版 `WHERE id IN (SELECT id FROM users FOR UPDATE)` 每小时
    // 对全表行加锁，与所有写余额的 API 事务互斥（用户量大时持锁秒级）。改为只重算
    // 本小时流水覆盖到的用户 —— sum(ledger) 结果与其余用户现有快照一致，语义不变。
    // 深测四轮（2026-10-03）：排除 kind='orphan_offset' 系统对冲行——0266 迁移把
    // 历史孤儿流水对冲 -52374 挂在 root 名下（对账口径是全站合计，个人重算不该吃），
    // 否则首个重算周期就把 root 快照打成大负数（实测 -51619，游戏/签到跟着报负）。
    sqlx::query(
        "UPDATE users SET spark_balance = COALESCE(( \
             SELECT base_spark FROM balance_baseline \
             WHERE user_id = users.id), 0) \
           + COALESCE(( \
             SELECT sum(amount) FROM spark_ledger \
             WHERE user_id = users.id \
               AND kind <> 'orphan_offset'), 0) \
         WHERE id IN (SELECT DISTINCT user_id FROM spark_ledger WHERE created_at > now() - interval '2 hours')",
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}
