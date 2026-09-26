-- 0218：G6 运行日志出口 + G7 任务面板全量（触发 worker 本体）
--
-- G6 现状：/admin/syslog 实读 audit_log（那是「谁做了什么」的操作审计，
--   已有 ?tool=audit 页承载），站长在站点报错时没有任何运行日志可查。
--   处理：api/worker 各挂一个 tracing Layer，WARN+ 事件批量落 runtime_logs
--   （默认 warn，FLUX_RTLOG_LEVEL 可调）；/admin/syslog 改读本表；
--   保留期由 worker 的 purge_runtime_logs 维护（14 天 / 20 万行硬顶）。
--
-- G7 现状：任务面板只覆盖 10 个 job（run2 是「等价 SQL 复刻」），漏跑补偿
--   走复刻 SQL 而非 worker 本体，口径会随 job 演进漂移；且 Cron 节奏不可视。
--   处理：job_status = worker 任务目录（节奏）+ 最近一次运行结果（with_lock
--   统一登记，自动/手动共用）；job_triggers = 手动触发入队队列，worker 在
--   60s tick 内认领执行 —— 与定时任务同一函数、同一把 advisory 锁。

-- ============ G6：运行日志 ============
CREATE TABLE runtime_logs (
  id      bigserial PRIMARY KEY,
  ts      timestamptz NOT NULL DEFAULT now(),
  level   text NOT NULL,
  source  text NOT NULL,              -- api / worker（进程来源）
  target  text NOT NULL DEFAULT '',   -- tracing target（模块）
  message text NOT NULL,
  repeat  int  NOT NULL DEFAULT 1     -- 同批连续重复折叠计数
);
CREATE INDEX runtime_logs_ts_idx ON runtime_logs (ts DESC);
CREATE INDEX runtime_logs_level_ts_idx ON runtime_logs (level, ts DESC);

-- ============ G7：任务目录 + 手动触发队列 ============
CREATE TABLE job_status (
  job              text PRIMARY KEY,
  cadence          text NOT NULL DEFAULT '',   -- 调度节奏：60s/10m/30m/1h/6h/24h
  last_started_at  timestamptz,
  last_finished_at timestamptz,
  last_ok          boolean,
  last_result      text,                       -- 失败原因（成功为空）
  updated_at       timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE job_triggers (
  id           bigserial PRIMARY KEY,
  job          text NOT NULL,
  status       text NOT NULL DEFAULT 'pending', -- pending/running/done/failed
  requested_by bigint REFERENCES users(id) ON DELETE SET NULL,
  requested_at timestamptz NOT NULL DEFAULT now(),
  started_at   timestamptz,
  finished_at  timestamptz,
  ok           boolean,
  result       text
);
CREATE INDEX job_triggers_pending_idx ON job_triggers (status)
  WHERE status = 'pending';
CREATE INDEX job_triggers_job_idx ON job_triggers (job, id DESC);

-- 面板条目更名：该页从「系统日志（实为操作审计）」改为真运行日志
UPDATE staff_panel_entries
SET name = '运行日志', info = '查看 api/worker 运行告警与错误'
WHERE tab_key = 'syslog';
