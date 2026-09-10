-- 0027: PM 管理组（contactstaff 复刻）+ 任务系统扩展（tier 卡片口径）+ 保种区统计
-- 任务等级卡（包子站 task.php 五档：Master/Ultimate/Extreme/Veteran/Insane）
ALTER TABLE tasks ADD COLUMN IF NOT EXISTS tier TEXT;                 -- master/ultimate/extreme/veteran/insane
ALTER TABLE tasks ADD COLUMN IF NOT EXISTS subtitle TEXT;             -- 副标题（骨灰（原黄星及VIP专享））
ALTER TABLE tasks ADD COLUMN IF NOT EXISTS fee BIGINT NOT NULL DEFAULT 0;       -- 报名费
ALTER TABLE tasks ADD COLUMN IF NOT EXISTS duration_days INT NOT NULL DEFAULT 30; -- 任务期限
ALTER TABLE tasks ADD COLUMN IF NOT EXISTS quota_total INT NOT NULL DEFAULT 200; -- 名额上限
ALTER TABLE tasks ADD COLUMN IF NOT EXISTS sort INT NOT NULL DEFAULT 0;

-- 任务商店（包子站任务商店口径）
CREATE TABLE IF NOT EXISTS task_shop_items (
  id SERIAL PRIMARY KEY,
  name TEXT NOT NULL,                -- VIP
  span TEXT NOT NULL,                -- 九十天/三十天
  require_tier TEXT NOT NULL,        -- master
  require_count INT NOT NULL,        -- 12
  cost NUMERIC NOT NULL DEFAULT 0,   -- 费用（魔力）
  stock INT NOT NULL DEFAULT 0,      -- 库存
  sort INT NOT NULL DEFAULT 0
);

-- 五档任务种子（幂等：按 name 冲突跳过）
INSERT INTO tasks (name, tier, subtitle, metric, starts_at, ends_at, target_class, reward, penalty, fee, duration_days, quota_total, claim_limit, sort)
SELECT * FROM (VALUES
  ('Master', 'master', '骨灰（原黄星及VIP专享）', '{"upload_delta": 2147483648000, "download_delta": 644245094400, "seed_points_delta": 45000}'::jsonb, now() - interval '1 day', now() + interval '365 days', 0, 0, 0, 100000, 30, 200, 200, 1),
  ('Ultimate', 'ultimate', '走火入魔（原黄星及VIP专享）', '{"upload_delta": 1577058190400, "download_delta": 536870912000, "seed_points_delta": 40000}'::jsonb, now() - interval '1 day', now() + interval '365 days', 0, 0, 0, 80000, 30, 200, 200, 2),
  ('Extreme', 'extreme', '烧糊涂', '{"upload_delta": 1052266983900, "download_delta": 429496729600, "seed_points_delta": 35000}'::jsonb, now() - interval '1 day', now() + interval '365 days', 0, 0, 0, 50000, 30, 200, 200, 3),
  ('Veteran', 'veteran', '高烧', '{"upload_delta": 536870912000, "download_delta": 322122547200, "seed_points_delta": 30000}'::jsonb, now() - interval '1 day', now() + interval '365 days', 0, 0, 0, 30000, 30, 200, 200, 4),
  ('Insane', 'insane', '中烧', '{"upload_delta": 322122547200, "download_delta": 107374182400, "seed_points_delta": 20000}'::jsonb, now() - interval '1 day', now() + interval '365 days', 0, 15000, 0, 1000, 30, 200, 200, 5)
) AS seed(name, tier, subtitle, metric, starts_at, ends_at, target_class, reward, penalty, fee, duration_days, quota_total, claim_limit, sort)
WHERE NOT EXISTS (SELECT 1 FROM tasks WHERE tier IS NOT NULL);

-- 任务商店种子
INSERT INTO task_shop_items (name, span, require_tier, require_count, cost, stock, sort)
SELECT * FROM (VALUES
  ('VIP', '九十天', 'master', 12, 1.0, 74, 1),
  ('VIP', '三十天', 'ultimate', 6, 1.0, 38, 2),
  ('永久邀请一枚', '', 'extreme', 5, 1.0, 23, 3),
  ('修改用户名一次', '', 'veteran', 3, 1.0, 53, 4),
  ('射魔一次', '', 'veteran', 1, 1.0, 12, 5)
) AS seed(name, span, require_tier, require_count, cost, stock, sort)
WHERE NOT EXISTS (SELECT 1 FROM task_shop_items);
