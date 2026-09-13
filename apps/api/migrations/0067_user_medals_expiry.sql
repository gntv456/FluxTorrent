-- 勋章到期机制（社区板块审计 P2-1）：user_medals 此前无过期列、无清理任务，
-- 限时勋章（medals.duration_days）实际永久有效。
-- 授予/购买时按 duration_days 计算 expires_at；NULL = 永久。

ALTER TABLE user_medals
  ADD COLUMN IF NOT EXISTS expires_at TIMESTAMPTZ;

-- 存量行回填：按勋章 duration_days 以授予时间为基准补算（历史授予时间不可考的取迁移时刻）
UPDATE user_medals um
SET expires_at = um.granted_at + make_interval(days => COALESCE(m.duration_days, 0))
FROM medals m
WHERE m.id = um.medal_id
  AND m.duration_days IS NOT NULL
  AND m.duration_days > 0
  AND um.expires_at IS NULL;

-- 已过期勋章自动摘下佩戴位（过期不可继续佩戴展示）
UPDATE user_medals SET wearing = FALSE WHERE expires_at IS NOT NULL AND expires_at <= now();

CREATE INDEX IF NOT EXISTS idx_user_medals_expires ON user_medals (expires_at) WHERE expires_at IS NOT NULL;
