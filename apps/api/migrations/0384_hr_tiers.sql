-- 0384 H&R 分档要求表 + 做种减免（PT Studio FR-HR 对标）
--
-- 现状：H&R 要求是**每个种子一档**（torrents.hr_policy 的
-- seed_hours/days），对所有用户同一个数。没有分档，也没有任何
-- 「做种减免」概念。
--
-- 大站的成熟形态（评估报告 §2.1）：
--   1. **分档要求表**：不同身份（等级 / 管理员 / 发种数 / 注册天数）
--      对同一 种子的 H&R 要求不同；
--   2. **做种减免**：按额外做种量给减免——Gazelle 类大站「你多做了种，
--      你的 H&R 门槛就低一点」。这是护肝核心：裸比分享率会把老实做种
--      的人逼到删种；
--   3. **警告顺延**：新警告从现有警告到期时刻顺延（而非从今天重算，
--      避免连续违规被无限累加）。
--
-- 【关键设计：减免只降门槛、不降责任】
-- 减免后 required_seconds 仍 > 0。绝不能减到 0——那等于「做种多就
-- 不用 H&R」，会被刷种者直接利用。真正的豁免走 hr.exempt 权限位
-- （保种员/VIP）或 seedbox 的 hnr_exempt。

-- ① 分档定义：按 class_id 给不同档位
CREATE TABLE IF NOT EXISTS hr_tiers (
    tier      TEXT        PRIMARY KEY,
    name      TEXT        NOT NULL,
    -- 该档的做种小时要求（相对种子 hr_policy.seed_hours 的倍率，百分比）
    -- 100 = 不减免；60 = 只要求 60%；150 = 更严（给低等级用户的反向档）
    seed_hours_pct INT    NOT NULL DEFAULT 100
                            CHECK (seed_hours_pct BETWEEN 0 AND 400),
    -- 该档的考察天数倍率（100 = 不变）
    days_pct   INT        NOT NULL DEFAULT 100
                            CHECK (days_pct BETWEEN 10 AND 400),
    sort_order INT        NOT NULL DEFAULT 0
);

COMMENT ON TABLE hr_tiers IS
    'H&R 分档要求表（FR-HR 分档）。seed_hours_pct 是**倍率**而非绝对值——'
    '种子的 hr_policy 仍是基线，分档只在此之上加减，这样每个种子的'
    '个性化策略不必为每个等级重复配一遍。';

-- ② 等级 → 档位映射（未映射的等级走 default 档）
CREATE TABLE IF NOT EXISTS hr_class_tiers (
    class_id  INT  PRIMARY KEY,
    tier      TEXT NOT NULL REFERENCES hr_tiers(tier) ON DELETE CASCADE
);

COMMENT ON TABLE hr_class_tiers IS
    '等级 → H&R 档位。没映射的等级落 hr_tiers 里名为 default 的档；'
    '连 default 都没有则不减免（= 种子原值），绝不因配置缺失而放宽。';

-- ③ 做种减免阶梯：额外做种量 → 减免百分比
--    这是「护肝」的核心表：做得越多，门槛越低。
CREATE TABLE IF NOT EXISTS hr_seed_relief (
    id           BIGSERIAL PRIMARY KEY,
    -- 超过该额外做种小时数即享此档减免（小时，递增）
    extra_hours  INT    NOT NULL,
    -- 减免百分��（seed_hours_pct 的进一步下调，如 80 = 再降 20%）
    relief_pct   INT    NOT NULL CHECK (relief_pct BETWEEN 0 AND 100),
    UNIQUE (extra_hours)
);

CREATE INDEX IF NOT EXISTS idx_hr_seed_relief
    ON hr_seed_relief (extra_hours DESC);

COMMENT ON TABLE hr_seed_relief IS
    '做种减免阶梯（护肝核心）。取用户**本档周期内累计做种小时**超出'
    '种子 hr_policy.seed_hours 的部分，按本表查减免百分比。'
    '两条硬约束：减免后的门槛仍须 > 0（不把责任减到零）；'
    '多条命中时取**最靠后**的一档（阶梯递增，不叠加）。';

-- ④ 种子可忽略的默认种子数据（不配就是空表，行为不变）
INSERT INTO hr_tiers (tier, name, seed_hours_pct, days_pct, sort_order)
VALUES
    ('default', '默认档', 100, 100, 0),
    ('reduced', '减免档', 60, 100, 10),
    ('strict',  '严苛档', 150, 80, 20)
ON CONFLICT (tier) DO UPDATE SET
    name = EXCLUDED.name,
    seed_hours_pct = EXCLUDED.seed_hours_pct,
    days_pct = EXCLUDED.days_pct,
    sort_order = EXCLUDED.sort_order;

INSERT INTO hr_class_tiers (class_id, tier) VALUES
    (1, 'strict')   -- LV1 新芽：刚注册，严格要求能筛掉随手下载
ON CONFLICT (class_id) DO UPDATE SET tier = EXCLUDED.tier;

INSERT INTO hr_seed_relief (extra_hours, relief_pct) VALUES
    (100,  90),
    (500,  80),
    (2000, 70),
    (5000, 60)
ON CONFLICT (extra_hours) DO UPDATE SET relief_pct = EXCLUDED.relief_pct;

-- ⑤ 减免只降门槛不降责任：设下限（小时）。
--    低于 1 小时的 H&R 义务没有意义（announce 间隔都到不了），
--    但更重要的是「不为零」——刷种者最爱利用这一点。
INSERT INTO site_settings (name, value) VALUES
    ('hr_relief_floor_hours', '1')
ON CONFLICT (name) DO NOTHING;

COMMENT ON TABLE site_settings IS
    '站点配置。hr_relief_floor_hours：做种减免后的 H&R 门槛下限（小时），'
    '确保减免只降门槛不降责任。';
