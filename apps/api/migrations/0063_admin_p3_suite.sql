-- 第八轮 P3 管理套件（好学站后台逐页深挖对照报告落地）：
-- 1) 种子批量工作台字段（pos_state/pick_type，优惠复用 promotions scope='torrent'）
-- 2) 标签字典样式属性 + 分类模式作用域（好学 tags 口径）
-- 3) Section 多维体系（分类模式 + 七维字典 + 种子关联）
-- 4) 分类级自动过审开关（Auto Approval Settings）
-- 5) 改名记录 / 用户修改记录
-- 6) Tracker URL 多地址管理
-- 7) 权限项登记 + 角色授权 + 管理面板导航登记

-- ============ 1) 种子批量工作台字段（NP posState / picktype 口径） ============
ALTER TABLE torrents
  ADD COLUMN IF NOT EXISTS pos_state SMALLINT NOT NULL DEFAULT 0,      -- 0 普通 1 置顶
  ADD COLUMN IF NOT EXISTS pos_state_until TIMESTAMPTZ,                -- 置顶截止（NULL = 长期）
  ADD COLUMN IF NOT EXISTS pick_type SMALLINT NOT NULL DEFAULT 0;      -- 0 普通 1 推荐 2 经典
CREATE INDEX IF NOT EXISTS idx_torrents_pos_state ON torrents (pos_state, pos_state_until);
CREATE INDEX IF NOT EXISTS idx_torrents_pick_type ON torrents (pick_type);

-- ============ 2) 标签字典：样式属性 + 作用域 ============
ALTER TABLE tag_dict
  ADD COLUMN IF NOT EXISTS bg_color TEXT NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS color TEXT NOT NULL DEFAULT '#ffffff',
  ADD COLUMN IF NOT EXISTS font_size TEXT NOT NULL DEFAULT '12px',
  ADD COLUMN IF NOT EXISTS margin TEXT NOT NULL DEFAULT '0 4px 0 0',
  ADD COLUMN IF NOT EXISTS padding TEXT NOT NULL DEFAULT '1px 4px',
  ADD COLUMN IF NOT EXISTS border_radius TEXT NOT NULL DEFAULT '2px',
  ADD COLUMN IF NOT EXISTS sort INT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS enabled BOOLEAN NOT NULL DEFAULT TRUE,
  ADD COLUMN IF NOT EXISTS mode_id INT;

-- ============ 3) Section 多维体系 ============
-- 分类模式：每个模式选择启用哪些子维度（好学 sections 口径）
CREATE TABLE IF NOT EXISTS category_modes (
  id SERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  show_source BOOLEAN NOT NULL DEFAULT TRUE,
  show_medium BOOLEAN NOT NULL DEFAULT TRUE,
  show_codec BOOLEAN NOT NULL DEFAULT TRUE,
  show_audio_codec BOOLEAN NOT NULL DEFAULT TRUE,
  show_standard BOOLEAN NOT NULL DEFAULT TRUE,
  show_processing BOOLEAN NOT NULL DEFAULT TRUE,
  show_team BOOLEAN NOT NULL DEFAULT TRUE
);
INSERT INTO category_modes (id, name)
SELECT 1, '默认模式' WHERE NOT EXISTS (SELECT 1 FROM category_modes);
SELECT setval('category_modes_id_seq', GREATEST((SELECT max(id) FROM category_modes), 1));

ALTER TABLE categories ADD COLUMN IF NOT EXISTS mode_id INT REFERENCES category_modes(id);
UPDATE categories SET mode_id = 1 WHERE mode_id IS NULL;

-- 子维度字典：kind ∈ codec/audio_codec/standard/team/source/processing
-- （media/grade/edition 复用 0001 既有三表，管理端统一 CRUD）
CREATE TABLE IF NOT EXISTS section_dict (
  id      BIGSERIAL PRIMARY KEY,
  kind    TEXT NOT NULL CHECK (kind IN ('codec','audio_codec','standard','team','source','processing')),
  name    TEXT NOT NULL,
  sort    INT NOT NULL DEFAULT 0,
  mode_id INT REFERENCES category_modes(id)   -- NULL = 全模式可用
);
CREATE INDEX IF NOT EXISTS idx_section_dict_kind ON section_dict (kind, sort);

-- 种子 → 维度值（每维一条；medium/grade/edition 走 torrents 既有外键列）
CREATE TABLE IF NOT EXISTS torrent_sections (
  torrent_id BIGINT NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
  kind       TEXT NOT NULL,
  dict_id    BIGINT NOT NULL REFERENCES section_dict(id) ON DELETE CASCADE,
  PRIMARY KEY (torrent_id, kind)
);
CREATE INDEX IF NOT EXISTS idx_torrent_sections_dict ON torrent_sections (kind, dict_id);

-- 既有三字典补排序位
ALTER TABLE media   ADD COLUMN IF NOT EXISTS sort INT NOT NULL DEFAULT 0;
ALTER TABLE grades  ADD COLUMN IF NOT EXISTS sort INT NOT NULL DEFAULT 0;
ALTER TABLE editions ADD COLUMN IF NOT EXISTS sort INT NOT NULL DEFAULT 0;

-- ============ 4) 自动过审（分类级开关） ============
ALTER TABLE categories ADD COLUMN IF NOT EXISTS auto_approve BOOLEAN NOT NULL DEFAULT FALSE;

-- ============ 5) 改名记录 / 用户修改记录（好学 UsernameChangeLog / UserModifyLog 口径） ============
CREATE TABLE IF NOT EXISTS username_change_logs (
  id         BIGSERIAL PRIMARY KEY,
  uid        BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  old_name   TEXT NOT NULL,
  new_name   TEXT NOT NULL,
  operator   BIGINT REFERENCES users(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_rename_logs_uid ON username_change_logs (uid, created_at DESC);

CREATE TABLE IF NOT EXISTS user_modify_logs (
  id         BIGSERIAL PRIMARY KEY,
  uid        BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  modifier   BIGINT REFERENCES users(id),
  content    TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_modify_logs_uid ON user_modify_logs (uid, created_at DESC);

-- ============ 6) Tracker URL（NexusPHP tracker_urls 口径） ============
CREATE TABLE IF NOT EXISTS tracker_urls (
  id         SERIAL PRIMARY KEY,
  url        TEXT NOT NULL,
  is_default BOOLEAN NOT NULL DEFAULT FALSE,
  enabled    BOOLEAN NOT NULL DEFAULT TRUE,
  priority   INT NOT NULL DEFAULT 0,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============ 7) 权限项登记与授权 ============
INSERT INTO permissions (key, name, category, descr, sort) VALUES
  ('torrent.manage',    '种子管理',   'upload', '后台种子列表批量操作（置顶/优惠/推荐/标签/删除）', 15),
  ('hr.view',           'H&R 总览',   'seed',   '后台浏览 H&R 记录并批量豁免', 25),
  ('invite.view',       '邀请管理',   'user',   '后台浏览全站邀请码', 20),
  ('attendance.manage', '签到管理',   'user',   '后台签到流水与手工补签', 21),
  ('medal.manage',      '勋章管理',   'site',   '勋章字典 CRUD 与持有回收', 40),
  ('prop.manage',       '道具管理',   'site',   '道具 CRUD 与用户背包管理', 41),
  ('exam.manage',       '考核配置',   'site',   '考核岗位类型 CRUD', 42),
  ('task.manage',       '任务配置',   'site',   '任务定义 CRUD', 43),
  ('tracker.manage',    'Tracker URL','system', '多 announce 地址管理', 60)
ON CONFLICT (key) DO NOTHING;
UPDATE permissions SET implemented = TRUE
WHERE key IN ('torrent.manage','hr.view','invite.view','attendance.manage','medal.manage',
              'prop.manage','exam.manage','task.manage','tracker.manage');

INSERT INTO role_permissions (role_type, role_key, permission_key)
SELECT 'class', '90', key FROM permissions
WHERE key IN ('torrent.manage','hr.view','invite.view')
ON CONFLICT DO NOTHING;
INSERT INTO role_permissions (role_type, role_key, permission_key)
SELECT 'class', '93', key FROM permissions
WHERE key IN ('attendance.manage','medal.manage','prop.manage','exam.manage','task.manage')
ON CONFLICT DO NOTHING;
INSERT INTO role_permissions (role_type, role_key, permission_key)
SELECT 'class', '99', key FROM permissions WHERE key IN ('tracker.manage')
ON CONFLICT DO NOTHING;

-- ============ 8) 管理面板导航登记 ============
INSERT INTO staff_panel_entries (panel, name, url, info, sort, section, min_class, tab_key) VALUES
  ('moderator', 'H&R 管理',   '/admin?tool=hr',         'H&R 记录总览与批量豁免',       4,  'moderation', 90, 'hr'),
  ('moderator', '邀请管理',   '/admin?tool=invites',    '全站邀请码浏览',               11, 'users',      90, 'invites'),
  ('moderator', '用户记录',   '/admin?tool=userlogs',   '改名与资料修改记录',           12, 'users',      90, 'userlogs'),
  ('admin',     '签到记录',   '/admin?tool=attendance', '签到流水与手工补签',           13, 'users',      93, 'attendance'),
  ('admin',     '标签管理',   '/admin?tool=tagdict',    '标签字典与样式配置',           9,  'content',    93, 'tagdict'),
  ('admin',     '维度管理',   '/admin?tool=sections',   '分类模式与多维字典',           10, 'content',    93, 'sections'),
  ('admin',     '勋章管理',   '/admin?tool=medals',     '勋章字典与持有管理',           11, 'content',    93, 'medals'),
  ('admin',     '道具管理',   '/admin?tool=props',      '道具字典与用户背包',           7,  'ops',        93, 'props'),
  ('admin',     '考核配置',   '/admin?tool=exams',      '考核岗位类型管理',             8,  'ops',        93, 'exams'),
  ('admin',     '任务配置',   '/admin?tool=tasks',      '任务定义管理',                 9,  'ops',        93, 'tasks'),
  ('sysop',     'Tracker URL','/admin?tool=trackers',   '多 announce 地址管理',         13, 'system',     99, 'trackers');
