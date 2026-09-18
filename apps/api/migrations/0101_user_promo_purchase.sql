-- 0101：用户自购置顶/限时免费（好学站插件口径，P2 需求补全）。
-- 站方定价表：档位（sticky1 一级置顶 / sticky2 二级置顶 / free 限时免费）× 时长（小时）→ 魔力价。
-- 用户在种子详情页购买；写 torrents.pos_state/pos_state_until（置顶）或 promotions（免费），
-- 扣费走 spark_ledger 统一流水（幂等键 sticky-buy:{user}:{torrent}:{nonce}）。
CREATE TABLE IF NOT EXISTS promo_purchases (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    torrent_id BIGINT NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('sticky1', 'sticky2', 'free')),
    hours INT NOT NULL,
    price BIGINT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    starts_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ends_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_promo_purchases_torrent ON promo_purchases (torrent_id, ends_at DESC);

-- 默认价目（站方可改 site_settings 覆盖单项；键：promo_price.{kind}.{hours}）
INSERT INTO site_settings (name, value, grp, descr) VALUES
    ('promo_price.sticky1.24',  '5000', 'main', '一级置顶 24h（魔力）'),
    ('promo_price.sticky1.72',  '13000', 'main', '一级置顶 72h（魔力）'),
    ('promo_price.sticky2.24',  '2500', 'main', '二级置顶 24h（魔力）'),
    ('promo_price.sticky2.72',  '6500', 'main', '二级置顶 72h（魔力）'),
    ('promo_price.free.24',     '8000', 'main', '限时免费 24h（魔力）'),
    ('promo_price.free.72',     '20000', 'main', '限时免费 72h（魔力）')
ON CONFLICT (name) DO UPDATE SET descr = EXCLUDED.descr;

-- 功能开关（no = 详情页隐藏购买入口）
INSERT INTO site_settings (name, value, grp, descr) VALUES
    ('module_promo_buy', 'yes', 'main', '开放用户自购置顶/限时免费')
ON CONFLICT (name) DO NOTHING;
