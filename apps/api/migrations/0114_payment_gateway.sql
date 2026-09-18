-- 0114: 通用建站系统 U4 §12.1 —— 支付网关抽象（单一网关脚手架：易支付协议族）
--
-- 设计（策划案 §12.1）：
--   payment_provider   = none（默认，现状=通道关闭）/ epay（易支付协议族：通用商户网关）
--   payment_gateway_url / payment_pid / payment_key（secret：GET 回掩码）
--   payment_currency   = CNY（面值币种，站点可改 USD——展示口径）
-- 凭证由站长在设置中心填入即可，代码不依赖具体商户。

-- 1) 支付订单表：一次充值一条订单，回调按 order_no 幂等入账（复用 donation_ledger）
CREATE TABLE IF NOT EXISTS payment_orders (
    id          BIGSERIAL PRIMARY KEY,
    order_no    TEXT NOT NULL UNIQUE,          -- 站内单号（幂等键）：flux-{uid}-{seq}
    user_id     BIGINT NOT NULL REFERENCES users(id),
    amount_usd  NUMERIC NOT NULL,              -- 面值（USD 口径，与套餐对齐）
    amount_paid NUMERIC,                       -- 实付（网关币种，回调回填）
    channel     TEXT NOT NULL DEFAULT 'epay',  -- alipay/wechat（传给网关的支付方式）
    status      TEXT NOT NULL DEFAULT 'pending', -- pending / paid / failed
    trade_no    TEXT,                          -- 网关流水号（回调回填）
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    paid_at     TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_payment_orders_user ON payment_orders (user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_payment_orders_status ON payment_orders (status) WHERE status = 'pending';

-- 2) 设置键（payment_provider 默认 none = 通道未开通，T3：现状）
INSERT INTO site_settings (name, value, descr, grp) VALUES
('payment_provider', 'none', '支付网关：none / epay（易支付协议族）', 'anticheat')
ON CONFLICT (name) DO NOTHING;
-- grp 借 antichehat 分区不合适——修正到独立 payment 分区（下方 meta 同组）
UPDATE site_settings SET grp = 'payment' WHERE name = 'payment_provider';

INSERT INTO site_settings (name, value, descr, grp) VALUES
('payment_gateway_url', '', '易支付网关地址（例：https://pay.example.com）', 'payment'),
('payment_pid', '', '易支付商户 ID', 'payment'),
('payment_key', '', '易支付商户密钥', 'payment'),
('payment_currency', 'CNY', '收款币种（展示口径：CNY/USD）', 'payment')
ON CONFLICT (name) DO NOTHING;

-- 3) settings_meta 登记（secret 型密钥 GET 回掩码；payment 分区）
INSERT INTO settings_meta (name, type, label_zh, label_en, options, secret, group_key, card_order) VALUES
('payment_provider', 'enum', '支付网关', 'Payment provider',
 '{"options": ["none", "epay"]}'::jsonb, false, 'payment', 1),
('payment_gateway_url', 'text', '网关地址', 'Gateway URL', NULL, false, 'payment', 2),
('payment_pid', 'text', '商户 ID', 'Merchant ID', NULL, false, 'payment', 3),
('payment_key', 'password', '商户密钥', 'Merchant key', NULL, true, 'payment', 4),
('payment_currency', 'text', '收款币种', 'Currency', NULL, false, 'payment', 5)
ON CONFLICT (name) DO UPDATE
  SET type = EXCLUDED.type, label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
      options = EXCLUDED.options, secret = EXCLUDED.secret,
      group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;
