-- 0385 H&R 分档/减免的 SQL 函数面
--
-- 0384 建了表与 Rust 侧的 domain::hr 判定。但 hr_snapshot 的 INSERT ...
-- SELECT 是一整条 SQL，要在**按行**语境下按用户等级与做种量算减免——
-- 逐行调 Rust 不现实（要把整批读进内存再写回）。
--
-- 故这里把同一套口径落成两个 SQL 函数：Rust 侧 domain::hr 仍是唯一
-- 的判定事实源（单测钉住边界），SQL 侧是它在「按行求值」语境下的
-- 等价实现。两边必须同口径，故：
--   * 两边都用**倍率**语义（种子 hr_policy 是基线）；
--   * 两边都**只降门槛不降责任**（地板 hr_relief_floor_hours）；
--   * 两边都**阶梯取最靠后一档不叠加**。
--
-- ⚠️ 为什么不用 plpgsql 直接算而不在 Rust 里预聚合：
--   减免依赖「该用户在本档周期内的累计做种小时」，这是**全站聚合量**。
--   要么在 SQL 里算（LATERAL 子查询 / 函数），要么每轮全表跑一遍 Rust。
--   前者天然随查询走、索引可用；后者在 42 万 snatches 上是几十秒。
--
-- 纪律：全部 IMMUTABLE/STABLE + 幂等（CREATE OR REPLACE）。

-- ① 取该等级的分档倍率（未映射 → 100/100，即不减免）
CREATE OR REPLACE FUNCTION hr_tier_pct(p_class_id INT)
RETURNS TABLE(seed_hours_pct INT, days_pct INT)
LANGUAGE sql
STABLE
AS $$
    SELECT COALESCE(
        (SELECT t.seed_hours_pct FROM hr_class_tiers ct
         JOIN hr_tiers t ON t.tier = ct.tier
         WHERE ct.class_id = p_class_id),
        100),
        COALESCE(
        (SELECT t.days_pct FROM hr_class_tiers ct
         JOIN hr_tiers t ON t.tier = ct.tier
         WHERE ct.class_id = p_class_id),
        100);
$$;

COMMENT ON FUNCTION hr_tier_pct(INT) IS
    '等级 → H&R 分档倍率。未映射/配置缺失一律回 100（= 不减免）——'
    '绝不因配置漏填而放宽，那等于给所有人发免死金牌。';

-- ② 做种减免：按额外做种小时取**最靠后**的一档（不叠加）
CREATE OR REPLACE FUNCTION hr_relief_pct(p_extra_hours INT)
RETURNS INT
LANGUAGE sql
STABLE
AS $$
    SELECT COALESCE((
        SELECT r.relief_pct FROM hr_seed_relief r
        WHERE p_extra_hours >= r.extra_hours
        ORDER BY r.extra_hours DESC
        LIMIT 1), 0);
$$;

COMMENT ON FUNCTION hr_relief_pct(INT) IS
    '额外做种小时 → 减免百分比。取 extra_hours 最大的一档；'
    '多条命中**不叠加**——叠加会让「做种多」变成指数收益。';

-- ③ 减免后的做种小时（分档 × 减免，再钳地板）
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
        ( COALESCE((p_base_hours * (SELECT seed_hours_pct
                                   FROM hr_tier_pct(p_class_id))), 0)
          * (100 - hr_relief_pct(
                GREATEST(p_user_hours - p_base_hours, 0))) / 100
        )::int
    );
$$;

COMMENT ON FUNCTION hr_reduced_seed_hours(INT, INT, INT) IS
    '分档 + 减免后的 H&R 做种小时。与 domain::hr::requirement 同口径：'
    '先乘分档倍率，再乘减免余量，最后钳地板（绝不减到 0）。';

-- ④ 减免后的考察天数（分档 × 倍率，至少 1 天）
CREATE OR REPLACE FUNCTION hr_reduced_days(
    p_base_days INT,
    p_class_id  INT
)
RETURNS INT
LANGUAGE sql
STABLE
AS $$
    SELECT GREATEST(
        1,
        (p_base_days * (SELECT days_pct FROM hr_tier_pct(p_class_id))) / 100
    );
$$;

COMMENT ON FUNCTION hr_reduced_days(INT, INT) IS
    '分档后的考察天数。至少 1 天——0 天义务等于「买了但不用还」。'
    '注意这里必须**先乘后除再钳下限**，写成 max(1)/100 会算出 0。';
