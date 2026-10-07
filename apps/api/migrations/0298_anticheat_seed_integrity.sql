-- 0298（保种组实测审计 2026-10-07，报告 _doc/保种组视角实测审计-2026-10-07.md）
-- 反作弊落地：P0 基线重置套利 + P0 幽灵做种 + P1 速率缺省降档 + P1 累进处置。
-- 全部幂等（IF NOT EXISTS / ON CONFLICT DO NOTHING/UPDATE），重跑安全。

-- ============ P1：可入账速率上限缺省从 2 GiB/s（≈16 Gbps，现实不存在）降到 125 MiB/s（1 Gbps） =
-- 审计实测：旧缺省下单账号单种子稳定刷 120 GiB/分钟。只收敛「仍是出厂缺省值」的站点——
-- 站长显式改过（值 ≠ 2147483648）的尊重其选择，不动。
UPDATE site_settings SET value = '134217728'
WHERE name = 'traffic_credit_max_bps' AND value = '2147483648';

-- ============ P0-2：snatches 记录 announce 上报端口（幽灵做种判定的数据基础） =
-- seeded 幽灵判定 = seeding AND port>0 AND connectable<>0（代码侧），列在此落。
ALTER TABLE snatches ADD COLUMN IF NOT EXISTS last_port integer NOT NULL DEFAULT 0;
COMMENT ON COLUMN snatches.last_port IS
'最近一次 announce 上报的监听端口（0=客户端未开监听）。幽灵做种判定基础：port=0 不可连接，不参与做种收益/保种区/复活任务结算。';

-- ============ P1：cheat_events 处置状态（累进处置的「已处置」锚点） =
ALTER TABLE cheat_events ADD COLUMN IF NOT EXISTS resolved_at timestamptz;
COMMENT ON COLUMN cheat_events.resolved_at IS '管理组处置时点；NULL=未处置。cheat_enforce 累进处置只统计未处置事件。';

-- ============ P1：速率上限的元数据口径同步（min 保持 1MiB/s，缺省提示更新） =
UPDATE settings_meta SET hint =
  'announce 增量按「距上次上报的秒数 × 本值」为上限入账，超出部分只记事件不计流量，'
  '堵死「计数器回放刷上传量」。缺省 125 MiB/s（1 Gbps 线路的合理上限）；'
  '机房高上行站点可上调，防刷站建议下调到实际可达带宽。'
WHERE name = 'traffic_credit_max_bps' AND NOT readonly;
