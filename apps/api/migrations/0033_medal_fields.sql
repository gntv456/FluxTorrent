-- 0033 勋章字段补齐（好学站 Medal 模型口径）+ 魔力流水时间线
-- 参考 hxpt App\Models\Medal fillable：
--   name/description/price/duration/get_type/sale_begin_time/sale_end_time/
--   inventory/bonus_addition_factor/category_id
ALTER TABLE medals
    ADD COLUMN IF NOT EXISTS description text,
    ADD COLUMN IF NOT EXISTS duration_days int,
    ADD COLUMN IF NOT EXISTS get_type smallint NOT NULL DEFAULT 1, -- 1 兑换 2 授予 3 合成
    ADD COLUMN IF NOT EXISTS sale_begin_at timestamptz,
    ADD COLUMN IF NOT EXISTS sale_end_at timestamptz,
    ADD COLUMN IF NOT EXISTS inventory int,          -- NULL = 不限量
    ADD COLUMN IF NOT EXISTS bonus_addition_factor numeric(5,2) NOT NULL DEFAULT 0, -- 魔力加成系数（%）
    ADD COLUMN IF NOT EXISTS category_id int NOT NULL DEFAULT 0; -- 0 = 未分组

-- 勋章分类（medal_category 口径）
CREATE TABLE IF NOT EXISTS medal_categories (
    id          SERIAL PRIMARY KEY,
    name        TEXT NOT NULL,
    sort        INT NOT NULL DEFAULT 0,
    reward_bonus BIGINT NOT NULL DEFAULT 0  -- 集齐奖励魔力
);

INSERT INTO medal_categories (id, name, sort, reward_bonus)
SELECT * FROM (VALUES
  (1, '节令勋章', 1, 2000),
  (2, '成就勋章', 2, 5000),
  (3, '限定勋章', 3, 10000)
) AS seed(id, name, sort, reward_bonus)
WHERE NOT EXISTS (SELECT 1 FROM medal_categories);
SELECT setval('medal_categories_id_seq', GREATEST((SELECT max(id) FROM medal_categories), 1));

-- 种子数据回填新字段
UPDATE medals SET
    description = CASE id
        WHEN 1 THEN '开站纪念勋章，仅授予首批注册用户。'
        WHEN 2 THEN '开学季先锋用户限定。'
        WHEN 3 THEN '暑期活动限定。'
        WHEN 4 THEN '春分节气限定勋章。'
        WHEN 5 THEN '夏至节气限定勋章。'
        WHEN 6 THEN '秋分节气限定勋章。'
        WHEN 7 THEN '冬至节气限定勋章。'
        WHEN 8 THEN '连续 30 天保种达成。'
        WHEN 9 THEN '单月发布 10 枚种子达成。'
        WHEN 10 THEN '累计签到 30 天达成。'
        WHEN 11 THEN '累计签到 100 天达成。'
        WHEN 12 THEN '农场等级达到 10 级。'
        WHEN 13 THEN '学习类种子下载达 50 枚。'
        ELSE '种子守护者：长期保种贡献。'
    END,
    duration_days = NULL::int,
    get_type = CASE WHEN id IN (1, 8, 9, 10, 11, 12, 13, 14) THEN 2 ELSE 1 END,
    inventory = CASE WHEN limited THEN 100 ELSE NULL END::int,
    bonus_addition_factor = CASE rarity
        WHEN 'legendary' THEN 10.00
        WHEN 'epic' THEN 5.00
        WHEN 'rare' THEN 2.00
        ELSE 0.00
    END,
    category_id = CASE
        WHEN id BETWEEN 4 AND 7 THEN 1   -- 节令
        WHEN id IN (8, 9, 12, 13, 14) THEN 2 -- 成就
        WHEN id IN (1, 2, 3, 10, 11) THEN 3  -- 限定
        ELSE 0
    END
WHERE description IS NULL;

-- 限定勋章销售期（已过销售期的保持 NULL 表示长期在售）
UPDATE medals SET sale_begin_at = now() - interval '10 days', sale_end_at = now() + interval '80 days'
WHERE limited = true AND sale_begin_at IS NULL;
