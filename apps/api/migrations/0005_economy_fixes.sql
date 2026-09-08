-- 经济模块修正（§8.5-5 只增不改原则的补丁迁移）
-- 1) attendance.streak/reward 是 INT4，查询侧按 i32 取
-- 2) 补建 pool_donations（0001 中遗漏建表）
CREATE TABLE IF NOT EXISTS pool_donations (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id),
  amount BIGINT NOT NULL,
  month CHAR(7) NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_pool_donations_month ON pool_donations (month);
