-- 0332_network_subscriptions.sql
-- 纪录片专项收官：厂牌订阅（对标 §7.1，与 0075 group_subscriptions 同范式）。
--
-- 本文件执行时 content_networks 必须已存在（下方 network_id 外键引用）。
-- 该表初版建于 0330，第三次撞号让号（6b4a635，2026-10-09）后挪至 0333
-- ——文件序上 0332 先于 0333，全新空库跑到这里时表还不存在 ⇒ 装机
-- 第一步迁移即崩（隔离栈实证，存量库不受影响：表早已在）。
-- 修复：此处前置一段与 0333 完全同构的 CREATE TABLE IF NOT EXISTS
-- （含 kind/norm_name 唯一约束与查询索引；数据回填仍归 0333——
-- 0333 的 INSERT 走 ON CONFLICT DO NOTHING，空库照常执行不重复）。
-- 已应用 0332 的存量库换 checksum：见
-- scripts/align_migration_checksums_0332_reorder.sql。
--
-- 厂牌订阅本身：用户看到 BBC 的纪录片合集后可以「有新片通知我」。
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

-- 前置：content_networks 实体表（与 0333 的建表段同构，勿在此处回填数据）
CREATE TABLE IF NOT EXISTS content_networks (
    id BIGSERIAL PRIMARY KEY,
    -- 实体来源维度：documentary=network，movie=studio，anime=studio …
    kind TEXT NOT NULL DEFAULT 'network',
    name TEXT NOT NULL,
    -- 归一化键（去空白小写）防同义重复
    norm_name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (kind, name)
);

CREATE INDEX IF NOT EXISTS idx_content_networks_kind
    ON content_networks (kind, norm_name);

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
