-- 0213：促销类型外置成可配表（服务「不偏向任何 PT 类型」定位）。
-- 背景：用户自购促销档位此前写死在 content_http/promo.rs（kind ∈ sticky1/sticky2/free，
--       hours ∈ 24/72），任意架站方想加「提升下载量」「求种高亮」等档位必须改代码。
-- 方案：promo_kinds 注册表声明「档位」；promo_kind_tiers 声明「档位 × 时长 → 价格」。
--       效果落地方式（effect）与计费倍率由 effect 字段驱动，前端/API/worker 读表。
--
-- effect 语义（与 worker billing 口径对齐）：
--   free       → promotions(kind='free')，下载计 0
--   x2         → promotions(kind='x2')，上传 ×2
--   x2free     → promotions(kind='x2free')，上传 ×2 下载 0
--   half       → promotions(kind='half')，下载 ×0.5
--   x2half     → promotions(kind='x2half')，上传 ×2 下载 ×0.5
--   p30        → promotions(kind='p30')，下载 ×0.3
--   sticky1    → torrents.pos_state=1（一级置顶）
--   sticky2    → torrents.pos_state=2（二级置顶）
CREATE TABLE IF NOT EXISTS promo_kinds (
    kind        TEXT PRIMARY KEY,
    label_zh    TEXT NOT NULL,
    label_en    TEXT NOT NULL DEFAULT '',
    -- 落地效果：sticky1/sticky2 = 写 pos_state；其余 = 写 promotions(kind=effect)
    effect      TEXT NOT NULL,
    -- 前台可见名走 i18n 字典键（缺省回落 label_zh）
    i18n_key    TEXT,
    sort_order  INT NOT NULL DEFAULT 100,
    enabled     BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS promo_kind_tiers (
    id          BIGSERIAL PRIMARY KEY,
    kind        TEXT NOT NULL REFERENCES promo_kinds(kind) ON DELETE CASCADE,
    hours       INT NOT NULL CHECK (hours > 0),
    price       BIGINT NOT NULL CHECK (price > 0),
    enabled     BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (kind, hours)
);
CREATE INDEX IF NOT EXISTS idx_promo_kind_tiers_kind
    ON promo_kind_tiers (kind, hours);

-- 默认种子 = 老站现状（sticky1/sticky2/free × 24/72），保证零行为变化。
-- 价目沿用 0101 的 site_settings 键；此处同步落一份到 tier 表作为权威读源。
INSERT INTO promo_kinds (kind, label_zh, label_en, effect, i18n_key, sort_order) VALUES
    ('sticky1', '一级置顶', 'Top Sticky (L1)', 'sticky1', 'sticky1', 10),
    ('sticky2', '二级置顶', 'Top Sticky (L2)', 'sticky2', 'sticky2', 20),
    ('free',    '限时免费', 'Freeleech',      'free',    'free',    30)
ON CONFLICT (kind) DO NOTHING;

-- 价目：优先取 site_settings 里的现值（站方可能已改价），缺省用 0101 默认价
INSERT INTO promo_kind_tiers (kind, hours, price)
SELECT k.kind, h.hours,
       COALESCE(
           (SELECT s.value::bigint FROM site_settings s
             WHERE s.name = format('promo_price.%s.%s', k.kind, h.hours)
               AND s.value ~ '^[0-9]+$'
               AND s.value::bigint > 0),
           d.price
       )
  FROM (VALUES ('sticky1'), ('sticky2'), ('free')) AS k(kind)
  CROSS JOIN (VALUES (24), (72)) AS h(hours)
  JOIN (VALUES
        ('sticky1', 24, 5000::bigint),
        ('sticky1', 72, 13000::bigint),
        ('sticky2', 24, 2500::bigint),
        ('sticky2', 72, 6500::bigint),
        ('free',    24, 8000::bigint),
        ('free',    72, 20000::bigint)
       ) AS d(kind, hours, price)
    ON d.kind = k.kind AND d.hours = h.hours
ON CONFLICT (kind, hours) DO NOTHING;

-- 老表 promo_purchases 的 kind CHECK 约束改为「外键到 promo_kinds」，
-- 这样站长新增档位后购买记录才能落库。
ALTER TABLE promo_purchases
    DROP CONSTRAINT IF EXISTS promo_purchases_kind_check;
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'promo_purchases_kind_fkey'
    ) THEN
        ALTER TABLE promo_purchases
            ADD CONSTRAINT promo_purchases_kind_fkey
            FOREIGN KEY (kind) REFERENCES promo_kinds(kind);
    END IF;
END $$;
