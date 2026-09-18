-- 0103：社交层补齐契约期限与信誉参数。
--
-- 背景：0102 建 social_team 时遗漏了 deadline_at（失败流转必需），
-- 且 social_reputation 表建好后一直没有读写逻辑。
-- 注意：0102 已在库中执行，**不能修改其内容**（sqlx 会校验 checksum，改动会导致
-- `migration 102 was previously applied but has been modified`），因此用本迁移补列。

-- ============ 契约期限 ============
ALTER TABLE social_team ADD COLUMN IF NOT EXISTS deadline_at TIMESTAMPTZ;

-- 只给「未处理」的队伍建索引：结算/失败后不再参与扫描
CREATE INDEX IF NOT EXISTS idx_social_team_open_deadline
    ON social_team (deadline_at) WHERE settled_at IS NULL;

-- 回填存量数据（0102 之后建的队伍都没有期限）
UPDATE social_team
   SET deadline_at = created_at + interval '14 days'
 WHERE deadline_at IS NULL;

-- ============ 信誉参数 ============
-- 设计口径（详见 _doc/契约失败流转与信誉.md）：
--   * 事实驱动为主：完成 / 失败 / 中途退出都由系统判定，不可伪造
--   * 不连坐：契约失败不惩罚个别成员，只记一次失败事实
--   * 只影响准入资格，不剥夺已得收益
INSERT INTO site_settings (name, value, grp, descr) VALUES
    ('social_team_default_days', '14',  'main', '组队契约默认期限（天）'),
    ('social_rep_on_fulfilled',  '20',  'main', '契约完成：信誉 +'),
    ('social_rep_on_failed',     '-5',  'main', '契约失败（到期未达标）：信誉变化'),
    ('social_rep_on_withdrawn',  '-15', 'main', '契约中途退出：信誉变化'),
    ('social_rep_min',           '0',   'main', '信誉下限（不设负值）'),
    ('social_rep_max',           '2000', 'main', '信誉上限')
ON CONFLICT (name) DO NOTHING;
