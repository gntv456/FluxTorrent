-- 0074 P1-3/P1-12 落地（对照 v3 §27-3 / §27-12）：
--   ① 收益公式改造（worker seeding_reward）无需 DDL——稀有度/饱和/arctan 全在 SQL 结算语句里。
--   ② 教材愿望单（U3D WishList 教育化）：订阅科目/年级/教材名，新种过审时匹配推送站内信。

CREATE SEQUENCE IF NOT EXISTS wishlist_id_seq;
CREATE TABLE IF NOT EXISTS wishlist (
    id BIGINT PRIMARY KEY DEFAULT nextval('wishlist_id_seq'),
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    keyword TEXT NOT NULL,                 -- 教材名/科目/任意关键词（trgm 匹配种子名+小描述）
    category_id INT REFERENCES categories(id),
    grade_id INT,                          -- 教育域维度（可空 = 不限）
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    notified_at TIMESTAMPTZ,               -- 最近一次命中推送时间（每条愿望 24h 限一条，防轰炸）
    UNIQUE (user_id, keyword)
);
CREATE INDEX IF NOT EXISTS idx_wishlist_user ON wishlist (user_id);

-- 触发点：种子过审（approval_status 0→1）。worker 每小时扫「过去 1 小时过审」的种子
-- 对 wishlist 做 (keyword ILIKE name/small_descr OR trgm 相似) AND (category/grade 可选匹配)，
-- 命中且 24h 未推 → 站内信（一信聚合多条命中，避免信箱轰炸）。

-- 过审时间戳（此前只有状态位，无法表达"刚过审"）：回填用 created_at 近似（存量），新审核由 API 写入。
ALTER TABLE torrents ADD COLUMN IF NOT EXISTS approved_at TIMESTAMPTZ;
UPDATE torrents SET approved_at = created_at WHERE approval_status = 1 AND approved_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_torrents_approved_at ON torrents (approved_at) WHERE approval_status = 1;
