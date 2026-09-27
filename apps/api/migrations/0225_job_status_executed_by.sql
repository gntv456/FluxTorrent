-- 0225：G30-B1 调度层多实例注记
--
-- 现状缺口（附录 D 档三）：job_status 主键 job 单行覆盖写，多 worker 下
--   「最近一次运行」被随机一台盖掉——面板看不出谁在跑、也无法分实例观察。
-- 处理：加 executed_by 记录最近一次执行实例（with_lock 内 mark_start 时写）。
--   历史化（每实例一行）仍列长期——本列先解决「谁在跑」的可见性。

ALTER TABLE job_status
  ADD COLUMN IF NOT EXISTS executed_by text NOT NULL DEFAULT '';
