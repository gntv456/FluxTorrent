-- 0079 v4 缺口批次（对照 v4 文档 G3/G5/G6/G7/G17/G18）：
--   A) 成就四族（U3D 口径简化版：保种/救种/发种/论坛）——definitions 表驱动 + 授予记录
--   B) 规则页版本化（Gazelle Wiki revision 口径：改规则自动存旧版）
--   C) 商店权益周期列：donor_until（免广告/VIP 待遇到期；已有 vip_until 复用）
--   D) 火花日度对账视图（G17：净增率常驻仪表的数据底座）

-- A) 成就定义 + 授予（worker 周期扫描授予；definitions 表驱动可运营加新）
CREATE TABLE IF NOT EXISTS achievement_defs (
    id BIGSERIAL PRIMARY KEY,
    family TEXT NOT NULL,              -- seeding 保种 / rescue 救种 / upload 发种 / forum 论坛
    code TEXT NOT NULL UNIQUE,         -- 稳定代码（如 seed_100g）
    name TEXT NOT NULL,
    descr TEXT NOT NULL DEFAULT '',
    metric TEXT NOT NULL,              -- SQL 侧统一口径的字段名（worker 按 family 聚合后比对）
    threshold BIGINT NOT NULL,
    reward_sparks BIGINT NOT NULL DEFAULT 0,
    position INT NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS user_achievements (
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    def_id BIGINT NOT NULL REFERENCES achievement_defs(id) ON DELETE CASCADE,
    metric_value BIGINT NOT NULL DEFAULT 0,   -- 授予时的指标快照
    granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, def_id)
);
-- 四族起步（数值对照 U3D 成就族的教育站收敛版）
INSERT INTO achievement_defs (family, code, name, descr, metric, threshold, reward_sparks, position) VALUES
  ('seeding', 'seed_vol_100g',  '保种学徒',   '做种总量 ≥ 100GiB',        'seeding_bytes', 107374182400,   500, 1),
  ('seeding', 'seed_vol_500g',  '保种行家',   '做种总量 ≥ 500GiB',        'seeding_bytes', 536870912000,  2000, 2),
  ('seeding', 'seed_vol_2t',    '保种宗师',   '做种总量 ≥ 2TiB',          'seeding_bytes', 2199023255552, 8000, 3),
  ('rescue',  'rescue_1',       '首度救种',   '完成复活任务 ≥ 1 个',      'rescue_count',  1,             1000, 1),
  ('rescue',  'rescue_10',      '救种达人',   '完成复活任务 ≥ 10 个',     'rescue_count',  10,            5000, 2),
  ('upload',  'up_1',           '初为人师',   '过审发布 ≥ 1 个',          'upload_count',  1,             200, 1),
  ('upload',  'up_50',          '桃李天下',   '过审发布 ≥ 50 个',         'upload_count',  50,           5000, 2),
  ('forum',   'post_10',        '论坛常客',   '发帖 ≥ 10',                'post_count',    10,             200, 1),
  ('forum',   'post_100',       '论坛元老',   '发帖 ≥ 100',               'post_count',    100,           2000, 2)
ON CONFLICT (code) DO NOTHING;

-- B) 规则修订历史（rule_update 时先把旧版落此表）
CREATE TABLE IF NOT EXISTS rules_revisions (
    id BIGSERIAL PRIMARY KEY,
    rule_id INT NOT NULL REFERENCES site_rules(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    sort INT NOT NULL,
    edited_by BIGINT REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()   -- 该版本的存档时间（=被替换时间）
);
CREATE INDEX IF NOT EXISTS idx_rules_revisions_rule ON rules_revisions (rule_id, created_at DESC);

-- C) 权益周期：donor_until（免广告档；VIP 用既有 users.vip_until）
ALTER TABLE users ADD COLUMN IF NOT EXISTS donor_until TIMESTAMPTZ;

-- D) 火花日度对账（G17：admin 净增率仪表；v3 的月度视图保持不变）
CREATE OR REPLACE VIEW v_spark_flow_daily AS
SELECT (created_at AT TIME ZONE 'Asia/Shanghai')::date AS day,
       SUM(CASE WHEN amount > 0 THEN amount ELSE 0 END) AS minted,
       SUM(CASE WHEN amount < 0 THEN -amount ELSE 0 END) AS burned,
       SUM(amount) AS net
FROM spark_ledger
WHERE created_at > now() - interval '90 days'
GROUP BY 1 ORDER BY 1;
