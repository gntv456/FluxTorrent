-- 0064：职务管理补全 + 导航重复修复
-- 背景（用户反馈）：①左侧导航出现两个「职务管理」——0054 与 0063 各插了一行
--   tab_key='roles'（当时 ON CONFLICT DO NOTHING 未指定唯一键，表上也没有约束，等于没防重）；
--  ②职务管理页只有「授予/撤销」，职务字典本身没有新增/编辑/删除。

-- ============ 1) 导航去重：补唯一约束（先清重） ============
DELETE FROM staff_panel_entries a
USING staff_panel_entries b
WHERE a.tab_key = b.tab_key
  AND a.tab_key <> ''            -- 外链型条目（如 settings）tab_key 可能语义重复，只约束工具型
  AND a.id > b.id;

CREATE UNIQUE INDEX IF NOT EXISTS uq_staff_panel_tab_key
  ON staff_panel_entries (tab_key) WHERE tab_key <> '';

-- ============ 2) 职务字典管理（roles CRUD） ============
-- 权限点沿用 staff.panel 所在档；字典操作仅站长（class 99）——与 user_roles 授予(93)区分：
-- 职务定义影响权限模型，收窄到 sysop。
INSERT INTO permissions (key, name, category, descr, sort) VALUES
  ('roles.manage', '职务定义管理', 'user', '职务字典的新增/编辑/删除（仅站长）', 22)
ON CONFLICT (key) DO NOTHING;
UPDATE permissions SET implemented = TRUE WHERE key = 'roles.manage';

INSERT INTO role_permissions (role_type, role_key, permission_key)
SELECT 'class', '99', 'roles.manage'
ON CONFLICT DO NOTHING;

-- 预置职务允许排序展示（roles 表 0054 已有 sort 列）
UPDATE roles SET sort = COALESCE(sort, 0);
