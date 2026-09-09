-- M24 好学农场（旧站 magic_fram.php 口径）
-- 农作物 5 天有效期；20% 概率双倍收获；市场价格每日 0/4/8/12/16/20 点刷新，波动 ±50%。
CREATE SEQUENCE IF NOT EXISTS farm_plots_id_seq;
CREATE SEQUENCE IF NOT EXISTS farm_harvests_id_seq;

CREATE TABLE IF NOT EXISTS farm_plots (
    id BIGINT PRIMARY KEY DEFAULT nextval('farm_plots_id_seq'),
    user_id BIGINT NOT NULL REFERENCES users(id),
    slot INT NOT NULL,                          -- 种植位（每用户 1..6）
    crop_id INT NOT NULL,                       -- farm_crops.id
    planted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ready_at TIMESTAMPTZ NOT NULL,              -- planted_at + grow_hours
    watered BOOLEAN NOT NULL DEFAULT FALSE,     -- 浇水 -10% 成熟时间（旧站口径）
    UNIQUE (user_id, slot)
);

CREATE TABLE IF NOT EXISTS farm_crops (
    id INT PRIMARY KEY,
    name TEXT NOT NULL,
    seed_price INT NOT NULL,                    -- 基准买入价（波动前）
    base_yield INT NOT NULL,                    -- 基础产量（火花/株）
    grow_hours INT NOT NULL
);

-- 收获流水（审计 + 市场统计）
CREATE TABLE IF NOT EXISTS farm_harvests (
    id BIGINT PRIMARY KEY DEFAULT nextval('farm_harvests_id_seq'),
    user_id BIGINT NOT NULL REFERENCES users(id),
    crop_id INT NOT NULL REFERENCES farm_crops(id),
    amount INT NOT NULL,                        -- 实得火花（含双倍/价格波动）
    market_price INT NOT NULL,                  -- 收获时市场价
    doubled BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO farm_crops (id, name, seed_price, base_yield, grow_hours) VALUES
    (1, '知识麦',  100,  80,  4),
    (2, '智慧豆',  300, 260, 12),
    (3, '学问瓜',  800, 720, 24),
    (4, '博士参', 2000, 1900, 48),
    (5, '状元稻', 5000, 4800, 96)
ON CONFLICT (id) DO UPDATE SET
    name = EXCLUDED.name,
    seed_price = EXCLUDED.seed_price,
    base_yield = EXCLUDED.base_yield,
    grow_hours = EXCLUDED.grow_hours;
