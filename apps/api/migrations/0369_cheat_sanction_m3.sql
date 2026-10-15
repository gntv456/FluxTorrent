-- 0369_cheat_sanction_m3.sql：反作弊处置与申诉闭环 M3
-- 处置可执行、可到期、可撤销；用户能申诉、staff 能四眼复核。
-- 幂等，可重跑。

-- ① reports.sanction：案件的处置记录（JSONB 状态机）
--   null                        = 未下达处置
--   {"state":"pending", ...}    = 高危处置（封禁类）待批准（四眼）
--   {"state":"applied", ...}    = 已生效（申诉 overturned 时据此反向解除）
--   kind: warn | ban | full_ban | reclaim
ALTER TABLE reports ADD COLUMN IF NOT EXISTS sanction jsonb;

-- ② appeals 加列（申诉闭环）
ALTER TABLE appeals ADD COLUMN IF NOT EXISTS token_hash text;   -- 被封用户令牌申诉（sha3-256，一次性）
ALTER TABLE appeals ADD COLUMN IF NOT EXISTS case_id bigint REFERENCES reports(id);
ALTER TABLE appeals ADD COLUMN IF NOT EXISTS outcome text;      -- upheld | mitigated | overturned
CREATE INDEX IF NOT EXISTS appeals_case ON appeals (case_id)
  WHERE case_id IS NOT NULL;

-- ③ users 加列：作弊停权（与 status=2 全封区分：禁做种收益+禁下载，
--    保留登录与申诉能力）+ 一次性申诉令牌（只存 hash）
ALTER TABLE users ADD COLUMN IF NOT EXISTS cheat_banned_until timestamptz;
ALTER TABLE users ADD COLUMN IF NOT EXISTS appeal_token_hash text;
ALTER TABLE users ADD COLUMN IF NOT EXISTS appeal_token_expires_at timestamptz;
-- 到期清理走此索引（worker cheat_enforce 每 10m 一把 UPDATE）
CREATE INDEX IF NOT EXISTS idx_users_cheat_banned
  ON users (cheat_banned_until) WHERE cheat_banned_until IS NOT NULL;

-- ④ cheat_events.false_positive：误判标记（overturned 时置位，
--    供 M4 规则准确率统计——误判率 = false_positive / 总结案数）
ALTER TABLE cheat_events ADD COLUMN IF NOT EXISTS false_positive boolean;

-- ⑤ 新权限（cheater 权限族两级：moderate 办案 / sanctions 下达处置）
INSERT INTO permissions (key, name, category, descr, sort) VALUES
('cheat.cases.moderate', '反作弊案件处理', 'staff', '认领/备注/关闭反作弊案件', 60),
('cheat.sanctions.manage', '作弊处置下达', 'staff', '对反作弊案件下达警告/停权/封禁/回收流量', 61)
ON CONFLICT (key) DO NOTHING;
-- 版主（class>=90）可办案；总版主（class>=93）才可下达处置（高危分离）
INSERT INTO role_permissions (role_type, role_key, permission_key)
SELECT 'class', '90', key FROM permissions WHERE key = 'cheat.cases.moderate'
ON CONFLICT DO NOTHING;
INSERT INTO role_permissions (role_type, role_key, permission_key)
SELECT 'class', '93', key FROM permissions WHERE key = 'cheat.sanctions.manage'
ON CONFLICT DO NOTHING;

-- ⑥ settings 种子
INSERT INTO site_settings (name, value, descr) VALUES
('cheat_ban_default_days', '30', '作弊停权默认天数（resolve sanction=ban 缺省值）'),
('appeal_token_days', '30', '被封用户申诉令牌有效期（天）')
ON CONFLICT (name) DO NOTHING;
