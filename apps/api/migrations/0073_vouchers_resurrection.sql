-- 0073 P1-5/P1-7 落地（对照 v3 §27-5 / §27-7）：
--   ① 免费券/中性券（Gazelle FL token 口径）：买→库存→用→该种下载免费；
--      核销规则抄 Gazelle——下载量超过种子大小 4% 即视为"已用"（防囤券转移后白嫖整种），
--      同时 30 天自然过期。
--   ② 复活任务（U3D Graveyard 口径）：保种区之外的死种（seeders=0 且 30 天无活）可领任务，
--      补种满 240h（可配）验收 → 奖火花 + 1 枚免费券 + 种子挂 7 天 free bump。

-- ① 券表：一人一券一行（每券绑定单种？否——先做"无绑定全券"，使用时选种核销）
CREATE SEQUENCE IF NOT EXISTS user_vouchers_id_seq;
CREATE TABLE IF NOT EXISTS user_vouchers (
    id BIGINT PRIMARY KEY DEFAULT nextval('user_vouchers_id_seq'),
    user_id BIGINT NOT NULL REFERENCES users(id),
    kind TEXT NOT NULL,                    -- free（免费券）| neutral（中性券：上下行均不计）
    source TEXT NOT NULL,                  -- shop | resurrection | activity
    granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT now() + interval '30 days',
    -- 使用即绑定（4% 核销口径需要知道"用在哪个种、当时下了多少"）
    used_torrent_id BIGINT REFERENCES torrents(id) ON DELETE SET NULL,
    used_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_user_vouchers_open ON user_vouchers (user_id, kind)
    WHERE used_torrent_id IS NULL AND used_at IS NULL;

-- 兼容已按旧结构建表的库（幂等）
ALTER TABLE user_vouchers ADD COLUMN IF NOT EXISTS used_torrent_id BIGINT REFERENCES torrents(id) ON DELETE SET NULL;
ALTER TABLE user_vouchers ADD COLUMN IF NOT EXISTS used_at TIMESTAMPTZ;

-- 商店上架（券是"买了就有库存"的消耗品，与装扮不同）
INSERT INTO shop_items (name, kind, price, config, active) VALUES
    ('免费券（单种下载免费）', 'voucher_free', 2500, '{"kind":"free"}', true),
    ('中性券（单种上下行均不计）', 'voucher_neutral', 5000, '{"kind":"neutral"}', true)
ON CONFLICT DO NOTHING;

-- ② 复活任务表
CREATE SEQUENCE IF NOT EXISTS resurrections_id_seq;
CREATE TABLE IF NOT EXISTS resurrections (
    id BIGINT PRIMARY KEY DEFAULT nextval('resurrections_id_seq'),
    torrent_id BIGINT NOT NULL UNIQUE REFERENCES torrents(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL REFERENCES users(id),
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    required_hours INT NOT NULL DEFAULT 240,   -- 领取时快照（site_settings resurrection_hours 可调）
    reward_sparks BIGINT NOT NULL DEFAULT 5000,
    status TEXT NOT NULL DEFAULT 'open',       -- open | done | expired
    finished_at TIMESTAMPTZ,
    UNIQUE (torrent_id)                        -- 一种一任务（领了别人不能再领）
);
CREATE INDEX IF NOT EXISTS idx_resurrections_open ON resurrections (status, claimed_at) WHERE status = 'open';
