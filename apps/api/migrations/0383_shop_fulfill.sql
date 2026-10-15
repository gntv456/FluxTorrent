-- 0383 商城三段式履约（PT Studio FR-SHOP-001~022 对标）
--
-- 现状（shop.rs）：`扣款 → 插订单 → CAS effect_applied → apply_item_effect`。
-- 这在正常路径够用，但**中间崩溃就没有回头路**：
--   * 扣完款、apply 中途失败 → 效果没发出去，钱没了。现有代码靠
--     「重放时检查 effect_applied=false 并补发」兜，但那要等用户
--     **自己重试**；不重试就永久「花钱买空气」；
--   * apply 前进程被杀 → 订单行 effect_applied 永远 false，没有任何
--     后台任务去推进这个中间态。
--
-- PT Studio 的三段式：**冻结金额 → 履约 → 扣款**，履约失败解冻。
-- 好处是「钱」在履约成功前一直是冻结态，任何中断都能安全回滚，
-- 且冻结本身对用户可见（「待确认订单」列表）。
--
-- 【与现有幂等口径的关系】不动 spend_spark / spark_ledger 的语义，
-- 只在其前面插一个 `frozen` 态：
--   reserving → frozen（已冻结，未履约）
--   fulfilled（已履约，扣款在同事务内完成）
--   failed / refunded（冻结解冻）
-- 旧的 `effect_applied` 列保留不动 —— 它仍是「效果是否到位」的判据，
-- 两条判据互补：新列管钱的阶段，老列管效果是否真的发出。

-- ① 订单补三段式状态机字段
ALTER TABLE shop_orders
    ADD COLUMN IF NOT EXISTS fulfill_state TEXT NOT NULL DEFAULT 'done',
    ADD COLUMN IF NOT EXISTS frozen_amount BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS frozen_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS fulfilled_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS fail_reason TEXT;

COMMENT ON COLUMN shop_orders.fulfill_state IS
    '三段式状态：reserving(占位中) → frozen(已冻结待履约) → done(已履约并扣款)；'
    '失败走 failed / refunded(冻结已解冻)。默认 done = 历史订单全部视为已完成。';
COMMENT ON COLUMN shop_orders.frozen_amount IS
    '冻结金额。履约成功时与 price 一起进 spend_spark；失败时按此额解冻。';
COMMENT ON COLUMN shop_orders.fail_reason IS
    '履约失败原因（排障用；不记 SQL 原文避免带出绑定数据）。';

-- 中间态扫查：恢复任务的唯一入口（不能全表扫 millions 行）
CREATE INDEX IF NOT EXISTS idx_shop_orders_stuck
    ON shop_orders (frozen_at)
    WHERE fulfill_state = 'frozen';

COMMENT ON INDEX idx_shop_orders_stuck IS
    '卡在 frozen 的订单（部分唯一：只索引未完成的行）。恢复任务按 frozen_at '
    '升序拉取，超过 grace 的重试履约或解冻——这是「崩溃后钱去哪了」的自愈口。';

-- ② 冻结台账：与 spark_ledger 同结构，便于统一对账
--    （不复用 spark_ledger 是因为那里只记已扣款；冻结是「还没花出去」）
CREATE TABLE IF NOT EXISTS shop_freezes (
    id           BIGSERIAL PRIMARY KEY,
    order_id     BIGINT      NOT NULL REFERENCES shop_orders(id) ON DELETE CASCADE,
    user_id      BIGINT      NOT NULL REFERENCES users(id),
    amount       BIGINT      NOT NULL,
    -- frozen=冻结中 / consumed=已扣款 / released=已解冻
    state        TEXT        NOT NULL DEFAULT 'frozen'
                             CHECK (state IN ('frozen', 'consumed', 'released')),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    settled_at   TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_shop_freezes_order
    ON shop_freezes (order_id);
CREATE INDEX IF NOT EXISTS idx_shop_freezes_stuck
    ON shop_freezes (created_at)
    WHERE state = 'frozen';

COMMENT ON TABLE shop_freezes IS
    '商城冻结台账（0383）。一行 = 一次冻结，与 spark_ledger 互补：'
    'ledger 记「已花出去」，这里记「还没花出去」。对账口径：'
    'frozen 未结 = 用户还有多少钱被站点占着（超 grace 由恢复任务处理）。';

-- ③ 站点级冻结宽限（秒）：超过即视为「履约卡住」，恢复任务介入。
--    默认 120s：足够 apply 走完（含外部调用），又不会让用户等太久。
INSERT INTO site_settings (name, value) VALUES
    ('shop_freeze_grace_secs', '120')
ON CONFLICT (name) DO NOTHING;

COMMENT ON TABLE site_settings IS
    '站点配置。shop_freeze_grace_secs：冻结后超过该秒数未完成，恢复任务'
    '判定为卡住并推进（重试履约或解冻退款）。';

-- ④ 递增价格（FR-SHOP-018）：限量商品的涨价阶梯
ALTER TABLE shop_items
    ADD COLUMN IF NOT EXISTS price_step BIGINT,
    ADD COLUMN IF NOT EXISTS price_max BIGINT,
    ADD COLUMN IF NOT EXISTS price_anchor_qty INT NOT NULL DEFAULT 0;

COMMENT ON COLUMN shop_items.price_step IS
    '递增价格：每卖出一份，已售量超过 price_anchor_qty 后单价 +price_step。'
    'NULL = 不递增（绝大多数商品）。';
COMMENT ON COLUMN shop_items.price_max IS
    '递增价格上限。超过后固定按此价卖——无限涨价会劝退，也会让有限量商品永远卖不完。';
