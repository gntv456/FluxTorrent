-- 0075 P1-10/P1-11 + P2 批次（对照 v3 §27-10 / §27-11 及 P2 高价值项）：
--   ① 聚合组补完：上传自动推荐入组（pieces_hash 命中直接锁定/名称 trgm 相似推荐）+ 组级订阅（新版本入组推送）
--   ② 通知偏好：user_notice_prefs JSONB（事件类→开关），发送侧（messages 写入点）过滤
--   ③ 站免池荣誉层（AB 池页口径：贡献榜公开）
--   ④ 免审积分制（NP offer_skip_approved_count 口径：连续 N 次通过免审）
--   ⑤ peer 超时分档（U3D 三档 TTL：活跃 ≥2×interval / 不活跃 90s——tracker 内存参数，此处只记口径）

-- ① 组级订阅
CREATE TABLE IF NOT EXISTS group_subscriptions (
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    group_id BIGINT NOT NULL REFERENCES torrent_groups(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, group_id)
);

-- ② 通知偏好（默认全开；JSONB 便于加事件类不改表）
ALTER TABLE users ADD COLUMN IF NOT EXISTS notice_prefs JSONB NOT NULL DEFAULT '{}';
-- 已知事件类清单（发送侧按类查 prefs.<key>=false 才跳过，缺省=发）：
--   hr_prewarn / hr_violation / wishlist / group_new_version / resurrection / class_promo
--   gift / comment_reply / message（私信本身不过滤） / system

-- ④ 免审积分：连续通过计数（被拒清零；达阈值 offer_skip 类用户免审）
ALTER TABLE users ADD COLUMN IF NOT EXISTS approve_streak INT NOT NULL DEFAULT 0;

-- ③ 站免池荣誉层视图（贡献榜：本月 + 累计）
CREATE OR REPLACE VIEW v_pool_honor AS
SELECT u.id, u.username, u.donor,
       COALESCE(SUM(CASE WHEN d.month = to_char(now() AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM')
                     THEN d.amount ELSE 0 END), 0) AS this_month,
       COALESCE(SUM(d.amount), 0) AS total
FROM users u
JOIN pool_donations d ON d.user_id = u.id
GROUP BY u.id, u.username, u.donor;
