-- 牧场 + 加工坊（样图⑤「动物牧场 / 加工坊」子页）：
-- 经济口径与农场一致——买牲畜/开加工是回收魔力，产出经 farm 收获同款
-- earn 入账（本批先落「pending 产出 + 手动收集」的简单环）。

-- 牲畜目录（站长可调）：成熟周期到点可收一次产物（魔力等值）
CREATE TABLE IF NOT EXISTS arcade_ranch_animals (
    key         text PRIMARY KEY,          -- chick / cow / sheep
    name        text NOT NULL,
    icon        text NOT NULL,
    price       bigint NOT NULL CHECK (price > 0),   -- 购入价（回收）
    yield_spark bigint NOT NULL CHECK (yield_spark > 0), -- 每次收集产出
    cycle_mins  integer NOT NULL CHECK (cycle_mins >= 10), -- 产出周期
    sort        integer NOT NULL DEFAULT 0,
    enabled     boolean NOT NULL DEFAULT true
);
INSERT INTO arcade_ranch_animals (key, name, icon, price, yield_spark, cycle_mins, sort) VALUES
    ('chick', '咕咕鸡', '🐔', 400,  40,  60, 10),
    ('cow',   '奶油牛', '🐄', 1200, 110, 180, 20),
    ('sheep', '云朵羊', '🐑', 800,  70, 120, 30)
ON CONFLICT (key) DO NOTHING;

-- 用户牲畜（每用户同种限一只：产物是固定回收环，多只=无限放大）
CREATE TABLE IF NOT EXISTS arcade_ranch_pens (
    user_id   bigint NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    animal    text   NOT NULL REFERENCES arcade_ranch_animals(key) ON DELETE CASCADE,
    ready_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, animal)
);

-- 加工配方（站长可调）：投入作物（按「等值魔力」计）+ 时间 → 产物魔力。
-- 不直接扣作物库存（站内作物收获即变现、无库存账），投入按 seed_price
-- 等值扣魔力 —— 与「作物是魔力凭证」的既有口径一致。
CREATE TABLE IF NOT EXISTS arcade_farm_recipes (
    key         text PRIMARY KEY,          -- butter / cheese / cake
    name        text NOT NULL,
    icon        text NOT NULL,
    in_spark    bigint NOT NULL CHECK (in_spark > 0),  -- 投入等值
    out_spark   bigint NOT NULL CHECK (out_spark > in_spark), -- 产出（回收侧 EV<1 由站长定档）
    mins        integer NOT NULL CHECK (mins >= 5),
    sort        integer NOT NULL DEFAULT 0,
    enabled     boolean NOT NULL DEFAULT true
);
INSERT INTO arcade_farm_recipes (key, name, icon, in_spark, out_spark, mins, sort) VALUES
    ('butter', '星尘黄油', '🧈', 100, 135, 10, 10),
    ('cheese', '月亮奶酪', '🧀', 300, 390, 30, 20),
    ('cake',   '美梦蛋糕', '🍰', 1000, 1240, 90, 30)
ON CONFLICT (key) DO NOTHING;

-- 用户加工位（每配方同时只有一个在产）
CREATE TABLE IF NOT EXISTS arcade_farm_crafts (
    user_id   bigint NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    recipe    text   NOT NULL REFERENCES arcade_farm_recipes(key) ON DELETE CASCADE,
    ready_at  timestamptz NOT NULL,
    out_spark bigint NOT NULL,             -- 开工时锁定产出（改配方不影响在产）
    PRIMARY KEY (user_id, recipe)
);
