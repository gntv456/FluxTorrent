-- 0394 H&R 分档倍率少除一个 100：义务时长被放大 100 倍
--
-- 现网取证（2026-10-19，全站账实体检顺出来的）：站点设定 hr_hours=1（9-25 起
-- 就是 1），而 10-18 之后新建的快照 required_seconds = 540000 秒 = **150 小时**
-- （10-16 之前建的快照是 3600 秒）。worker 日志同步表现成对：
-- 「H&R violations detected n=3」「违规降回阈值以下，已恢复下载权限 n=5」反复抖。
--
-- 根因在 0385 落下的 SQL 侧口径：分档倍率是「百分数」，必须先 /100 再乘减免余量，
-- 而函数写成
--     base * pct * (100 - relief) / 100
-- 只在最后除了一次 100 ⇒ 少除一个 100。Rust 侧同一口径是对的
-- （domain/hr/mod.rs:83  scaled = base * pct / 100），兄弟函数
-- hr_reduced_days 也是对的（(base_days * days_pct) / 100），只有这一处漏了。
-- 后果按档位分：
--   class 1 → strict（150%）：1 小时算成 150 小时
--   其它未映射档位（回 100%）：1 小时算成 100 小时 —— 人人中招，
--   因为 hr_class_tiers 目前只映射了 class 1，其余走 COALESCE(...,100)。
-- 建快照用的是这条 SQL（hr_snapshot.rs 的 INSERT...SELECT），所以真正落进
-- hr_snapshots.required_seconds 的就是这个放大值 ⇒ 成员大面积被误判 H&R 违规、
-- 下载权限被暂停（HR 是站方最硬的处置面之一）。
--
-- 修法两步：① CREATE OR REPLACE 把 /100 补回来（顺序与 Rust 侧逐字对齐：
-- 先 base*pct/100 得 scaled，再 scaled*(100-relief)/100，最后钳地板）；
-- ② 回填还没结清的快照（open/violated）用修正后的函数重算 required_seconds。
-- 状态位不动：达标/违规的推进交给 worker 下一轮（hr_resolve 会自动把误判期间
-- 被暂停的下载权限恢复，日志里已经在做这件事）。

CREATE OR REPLACE FUNCTION hr_reduced_seed_hours(
    p_base_hours  INT,
    p_class_id    INT,
    p_user_hours  INT
)
RETURNS INT
LANGUAGE sql
STABLE
AS $$
    SELECT GREATEST(
        -- 地板：减免只降门槛不降责任，缺省 1 小时
        COALESCE((SELECT value::int FROM site_settings
                  WHERE name = 'hr_relief_floor_hours'), 1),
        ( (COALESCE(p_base_hours * (SELECT seed_hours_pct
                                     FROM hr_tier_pct(p_class_id)), 0) / 100)
          * (100 - hr_relief_pct(
                GREATEST(p_user_hours - p_base_hours, 0))) / 100
        )::int
    );
$$;

COMMENT ON FUNCTION hr_reduced_seed_hours(INT, INT, INT) IS
    '分档 + 减免后的 H&R 做种小时。与 domain::hr::requirement 同口径：'
    '先乘分档倍率并除 100（0394 修的就是这一步少除了一次 100），'
    '再乘减免余量，最后钳地板（绝不减到 0）。';

-- ② 回填：只改还没结清的义务，已 satisfied/pardoned 的历史不动
WITH fixed AS (
    SELECT h.user_id, h.torrent_id,
           hr_reduced_seed_hours(
               COALESCE((t.hr_policy ->> 'seed_hours')::int, d.base_h),
               u.class_id,
               (SELECT COALESCE(sum(sx.seeded_seconds), 0)::bigint / 3600
                  FROM snatches sx
                 WHERE sx.user_id = h.user_id
                   AND sx.completed_at >= s.completed_at)::int
           )::bigint * 3600 AS req
      FROM hr_snapshots h
      JOIN torrents t ON t.id = h.torrent_id
      JOIN users u ON u.id = h.user_id
      JOIN snatches s ON s.user_id = h.user_id AND s.torrent_id = h.torrent_id
      CROSS JOIN LATERAL (
          SELECT COALESCE(NULLIF((SELECT value FROM site_settings
                                    WHERE name = 'hr_hours'), '')::int, 48)
                 AS base_h
      ) d
     WHERE h.status IN ('open', 'violated')
)
UPDATE hr_snapshots h
   SET required_seconds = f.req,
       updated_at = now()
  FROM fixed f
 WHERE h.user_id = f.user_id
   AND h.torrent_id = f.torrent_id
   AND h.status IN ('open', 'violated')
   AND h.required_seconds IS DISTINCT FROM f.req;
