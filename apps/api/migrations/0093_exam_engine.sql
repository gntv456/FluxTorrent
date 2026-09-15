-- 0093 考核引擎（exam）：tasks 单表方案（方案 §十一 P1-2）。
-- 复用 tasks + task_claims 全链路（领取/结算/发奖/PM/后台 CRUD），
-- 仅加三列把任务区分为普通任务与考核：
--   kind        'task'（默认，普通限时任务）| 'onboard'（新人转正）| 'periodic'（周期考核）
--   auto_assign 考核是否自动派发（注册后 N 天内的新人 / 满足 target_class 的用户）
--   period      'once'（默认，一次性）| 'monthly' | 'quarterly'（周期考核到期后可重派）

ALTER TABLE tasks
  ADD COLUMN IF NOT EXISTS kind TEXT NOT NULL DEFAULT 'task'
    CHECK (kind IN ('task', 'onboard', 'periodic')),
  ADD COLUMN IF NOT EXISTS auto_assign BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE tasks
  ADD COLUMN IF NOT EXISTS period TEXT NOT NULL DEFAULT 'once'
    CHECK (period IN ('once', 'monthly', 'quarterly'));

CREATE INDEX IF NOT EXISTS idx_tasks_auto_assign ON tasks (auto_assign) WHERE auto_assign;

-- 派发 PM 标记：exam_assign 新插认领后置 TRUE，防 worker 重跑重复发通知
ALTER TABLE task_claims
  ADD COLUMN IF NOT EXISTS pm_sent BOOLEAN NOT NULL DEFAULT FALSE;

-- 考核派发时间窗：onboard 面向注册 ≤ N 天新人（默认 30 天；按站调）
INSERT INTO site_settings (name, value, descr, grp) VALUES
  ('exam_onboard_days', '30', '新人转正考核自动派发窗口（注册后天数）', 'main')
ON CONFLICT (name) DO UPDATE SET descr = EXCLUDED.descr;

-- 种子：新人转正考核（做种 120 小时 = 432000 秒），自动派发、不限领
INSERT INTO tasks (name, subtitle, metric, starts_at, ends_at, target_class,
                   reward, penalty, claim_limit, duration_days, kind, auto_assign, period)
SELECT '新人转正考核', '注册后自动派发：做种满 120 小时即转正',
       '{"seed_seconds_delta": 432000}'::jsonb,
       now(), now() + interval '10 years', 0, 2000, 0, NULL, 365, 'onboard', TRUE, 'once'
WHERE NOT EXISTS (SELECT 1 FROM tasks WHERE name = '新人转正考核' AND kind = 'onboard');
