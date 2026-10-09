-- 0332_network_subscriptions.sql
-- 纪录片专项收官：厂牌订阅（对标 §7.1，与 0075 group_subscriptions 同范式）。
--
-- 0330 建了 content_networks 实体与厂牌页，但**订不了**——用户看到 BBC
-- 的纪录片合集后无法「有新片通知我」。成熟纪录片站（BBC Earth 档案站、
-- 纪录片之家）的订阅入口都在厂牌页上。
--
-- 承载评估（延续「轻结构、复用既有链路」纪律）：
--   · 与 group_subscriptions 完全同形（user_id + 目标 id 复合主键）；
--   · 通知复用既有的 messages + notice_prefs 机制（新增事件类
--     `network_new_release`），不新建通知通道；
--   · 目标用 (kind, network_id) 而非只有 id —— 同一张 content_networks
--     表要承载 documentary.network / movie.studio / anime.studio，
--     订阅必须能区分「订阅的是哪一维度的实体」。
--
-- 幂等：CREATE IF NOT EXISTS。

BEGIN;

CREATE TABLE IF NOT EXISTS network_subscriptions (
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- content_networks.id
    network_id BIGINT NOT NULL
        REFERENCES content_networks(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, network_id)
);

-- 发信侧按 network_id 批量取订阅者（过审副作用里一次查全部收件人）
CREATE INDEX IF NOT EXISTS idx_network_subs_network
    ON network_subscriptions (network_id);

COMMIT;
