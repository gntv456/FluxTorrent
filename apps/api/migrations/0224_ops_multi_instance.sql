-- 0224：G30-A2 多实例维度（附录 D 档二「排障分不开实例」项）
--
-- 现状缺口：runtime_logs.source 只有 'api'/'worker' 两个字面值，多副本部署下
--   「按来源筛日志」分不开机器；job_triggers 无认领者，两 worker 并发时面板
--   看不出哪台在跑。写入侧取 FLUX_INSTANCE_ID（默认容器 hostname），
--   单实例部署不设该变量时列为 ''，行为与以前一致（无筛选、无显示负担）。

ALTER TABLE runtime_logs
  ADD COLUMN IF NOT EXISTS instance text NOT NULL DEFAULT '';

ALTER TABLE job_triggers
  ADD COLUMN IF NOT EXISTS claimed_by text NOT NULL DEFAULT '';

CREATE INDEX IF NOT EXISTS runtime_logs_instance_ts_idx
  ON runtime_logs (instance, ts DESC);
