-- 0086: 种子付费下载（NP upload.php 价格 口径）
-- 发布者定价（0 = 免费，≤1,000,000）；下载者首次下载支付 price，
-- 发布者得 (100 - 税率)%，税入当月魔法池（与 pool_donate 同账）；
-- 已购/已付用户重复下载不再扣费（torrent_purchases 幂等表）。

ALTER TABLE torrents ADD COLUMN IF NOT EXISTS price BIGINT NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS torrent_purchases (
  user_id    BIGINT NOT NULL REFERENCES users(id),
  torrent_id BIGINT NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
  price      BIGINT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (user_id, torrent_id)
);

CREATE INDEX IF NOT EXISTS idx_torrent_purchases_torrent ON torrent_purchases (torrent_id);

-- 税率设置（%，发布者实得 = price * (100 - tax) / 100）
INSERT INTO site_settings (name, value, descr, grp) VALUES
('upload_price_tax', '30', '种子付费下载税率（%），税入当月魔法池', 'basic')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, card_order, min_class) VALUES
('upload_price_tax', 'number', '付费下载税率', 'Paid download tax', '下载付费种子的税率（%），发布者实得 = 价格 × (100 - 税率) ÷ 100；税入当月魔法池。0-90', '经济', 20, 99)
ON CONFLICT (name) DO UPDATE SET
    label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
    hint = EXCLUDED.hint, group_key = EXCLUDED.group_key,
    card_order = EXCLUDED.card_order, min_class = EXCLUDED.min_class;
