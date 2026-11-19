-- 0503 运营补全批（体检七轮「采纳建议全部完成」）：
--
-- 1) tasks.period 补 weekly 档：0093 的 CHECK 只有 once/monthly/quarterly
--    ——周常任务运营位做不了。exam.rs 重派逻辑按「上一认领已结算」判定，
--    weekly 与 monthly 同构，仅约束与白名单需放行。
-- 2) contests 补两列：prizes JSONB（名次→火花，结算 job 消费）与
--    settled_at（结算幂等锚）——此前 contests 是无建赛/无计分/无结算
--    的空转骨架，本批补 admin CRUD + 上传竞速结算引擎。

ALTER TABLE tasks DROP CONSTRAINT IF EXISTS tasks_period_check;
ALTER TABLE tasks ADD CONSTRAINT tasks_period_check
    CHECK (period IN ('once', 'weekly', 'monthly', 'quarterly'));

ALTER TABLE contests ADD COLUMN IF NOT EXISTS prizes JSONB
    NOT NULL DEFAULT '[]';   -- [{"rank":1,"sparks":5000},...]
ALTER TABLE contests ADD COLUMN IF NOT EXISTS settled_at TIMESTAMPTZ;

COMMENT ON COLUMN contests.prizes IS
  '名次奖励（结算 job 消费）：[{"rank":N,"sparks":M},...]';
