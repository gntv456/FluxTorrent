-- 0106 绩效考核重做（参考各 PT 站工作组考核口径）
--
-- 背景：旧实现的达标判定结构性失效——compute_metrics 只产出 6 个 key，
-- 而岗位种子数据用的 seed_days/seed_hours/seed_size_tb 全是死键（配了 required
-- 永远不达标）；ops 指标无月份过滤；bonus_rules 零消费（加成硬编码）；
-- 无月末结算（用户忘点领取 = 白干）；管理端只有单用户分配。
--
-- 本迁移：表结构补列 + 月度基线快照表（解决 seeded_seconds 是累计值无月维度）
-- + 配置项 + 种子岗位的 min_requirements 口径修复。

-- ============ 岗位类型：说明列 ============
ALTER TABLE jixiao_types ADD COLUMN IF NOT EXISTS description TEXT NOT NULL DEFAULT '';

-- ============ 考核登记/领取行：结算状态与基线 ============
-- status 语义（admin 登记行与用户领取行共用）：
--   0=进行中  1=达标已发薪  2=期末未达标  3=管理组撤销
ALTER TABLE jixiao_claims ADD COLUMN IF NOT EXISTS status SMALLINT NOT NULL DEFAULT 0;
ALTER TABLE jixiao_claims ADD COLUMN IF NOT EXISTS settled_at TIMESTAMPTZ;
ALTER TABLE jixiao_claims ADD COLUMN IF NOT EXISTS bonus_paid BIGINT NOT NULL DEFAULT 0;
ALTER TABLE jixiao_claims ADD COLUMN IF NOT EXISTS metrics_at_settle JSONB;
-- 基线快照（与 task_claims / seed_preserve / social_team_member 同款 base+delta 口径）：
-- admin 分配岗位时记录当刻累计值；结算期 = 现值 - 基线。
-- 期初无月度快照表的月份用它兜底（月度快照表是主口径，见下）。
ALTER TABLE jixiao_claims ADD COLUMN IF NOT EXISTS base_seed_seconds BIGINT NOT NULL DEFAULT 0;
ALTER TABLE jixiao_claims ADD COLUMN IF NOT EXISTS base_uploaded BIGINT NOT NULL DEFAULT 0;
ALTER TABLE jixiao_claims ADD COLUMN IF NOT EXISTS base_uploads BIGINT NOT NULL DEFAULT 0;

-- 未结算登记行的查询索引（worker 月末结算扫它）
CREATE INDEX IF NOT EXISTS idx_jixiao_claims_pending
  ON jixiao_claims (period) WHERE settled_at IS NULL AND status = 0;

-- ============ 月度基线快照表 ============
-- 每期结算后为下期落一行全站活跃用户的累计值快照；
-- 当月增量 = 现值 - 上一期快照。PRIMARY KEY (user_id, period) 天然幂等。
CREATE TABLE IF NOT EXISTS jixiao_baseline_snapshots (
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  period CHAR(7) NOT NULL,
  seed_seconds BIGINT NOT NULL DEFAULT 0,
  uploaded BIGINT NOT NULL DEFAULT 0,
  uploads BIGINT NOT NULL DEFAULT 0,
  taken_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (user_id, period)
);

-- ============ 配置项 ============
-- site_settings 的说明列叫 descr（不是 description）
INSERT INTO site_settings (name, value, descr) VALUES
  ('jixiao_claim_window_days', '7',  '绩效考核：月末结算后的补领窗口天数'),
  ('jixiao_bonus_months_per_step', '3', '绩效考核：每 N 个达标月触发一档加成'),
  ('jixiao_bonus_percent_per_step', '10', '绩效考核：每档加成百分比')
ON CONFLICT (name) DO UPDATE SET descr = EXCLUDED.descr;

-- ============ 种子岗位口径修复 ============
-- 判定统一只认 min_requirements（metrics 字段保留作展示参考）。
-- 旧种子岗位的 min_requirements 全为空（不设门槛人人可领），按原 metrics
-- 意图填上正确 key 的等值要求：
--   保种员   {seed_size_tb:5, seed_days:25} → {seed_hours:100, seed_size_tb:5}
--            （seed_days 是近似口径，考核主键改用精确的 seed_hours=100h≈25天×4h）
--   转种员   {uploads:50, seed_size_tb:2}   → 保持（uploads/seed_size_tb 均为活键）
--   发布员   {uploads:20}                    → 保持
--   维护开发员/主管 {ops:...}               → ops 修复为按月过滤后即活键，
--            要求值按 30 天折算（100→全历史无意义，月考核应按月计）
UPDATE jixiao_types SET min_requirements = '{"seed_hours": 100, "seed_size_tb": 5}'
WHERE name = '保种员' AND min_requirements = '{}';
UPDATE jixiao_types SET min_requirements = '{"ops": 30}'
WHERE name = '维护开发员' AND min_requirements = '{}';
UPDATE jixiao_types SET min_requirements = '{"ops": 120}'
WHERE name = '主管' AND min_requirements = '{}';
UPDATE jixiao_types SET min_requirements = '{"seed_hours": 50}'
WHERE name IN ('e2e考核岗') AND min_requirements = '{}';
-- 发布员/转种员的 uploads 键已活（当月发布数），补上防止旧库缺省
UPDATE jixiao_types SET min_requirements = '{"uploads": 20}'
WHERE name = '发布员' AND min_requirements = '{}';
UPDATE jixiao_types SET min_requirements = '{"uploads": 50, "seed_size_tb": 2}'
WHERE name = '转种员' AND min_requirements = '{}';

-- ============ 管理组面板入口 ============
-- 「考核配置」(exams) 现在只管岗位类型 CRUD（admin-exams）；
-- 新增「绩效考核」(jixiao) 面板：全站总览 / 批量分配 / 发薪记录（admin-jixiao）。
-- panel 列 NOT NULL 无默认值，沿用现有行的 'admin' 口径。
INSERT INTO staff_panel_entries (panel, section, name, url, info, tab_key, min_class, sort) VALUES
  ('admin', 'ops', '绩效考核', '#jixiao', '工作组考核总览 / 批量分配成员 / 发薪记录', 'jixiao', 93, 10)
ON CONFLICT DO NOTHING;
