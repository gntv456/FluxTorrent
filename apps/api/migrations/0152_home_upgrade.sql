-- 0152 首页对比调研落地（P0-2/P1-4）：
-- ① sticky_promotions 加 poster/starts 注入 Featured 海报轮播能力（UNIT3D featured-carousel 口径）
-- ② users 加 news_seen 公告已读水位（NP last_home 口径，前台轻量未读感知）
ALTER TABLE sticky_promotions
    ADD COLUMN IF NOT EXISTS poster TEXT,
    ADD COLUMN IF NOT EXISTS subtitle TEXT;

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS news_seen INT NOT NULL DEFAULT 0;
