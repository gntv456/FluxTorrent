-- 0217：G2 自建物权限细键 + G3 权限矩阵随模块开关收敛
--
-- G2 现状：sitepacks.manage 是「自建物一刀切」——自定义页面、用户自定义字段、
--   术语表与站型包/等级规则共用一个键，站长没法给「内容编辑」只开页面管理
--   （授单一职责必然附带站型包写权限）。
--   处理：拆出三个细键 + 把三个面板条目的 perm_key 从空改为对应细键；
--   授权**按现有持有者原样继承**（凡有 sitepacks.manage 的档/职务同授细键），
--   故对既有 98/99 档行为零漂移；细键缺省未授给任何档，只能站长显式授予。
--
-- G3 现状：权限矩阵把 68 个权限全量列出，模块关了（如商店）仍显示其专属权限
--   （prop.manage），配了也不生效。
--   处理：permissions 加 module_key（NULL = 不限模块），矩阵按开关过滤；
--   口径复用 modules::module_on_sql（单源，未配置键按关）——与面板条目、
--   导航菜单的模块过滤同一套语义。

-- ============ G2.1 细键登记 ============
INSERT INTO permissions (key, name, category, descr, sort, implemented) VALUES
('custompages.manage', '自定义页面管理', 'content', '自建页面的增删改（含导航/菜单可达性）', 50, true),
('userfields.manage',  '自定义字段管理', 'content', '用户自定义字段的定义与选项（注册/资料页展示）', 51, true),
('terms.manage',       '术语表管理',     'content', '站点术语改写规则的增删改（前端字典 + 后端校验串两出口）', 52, true)
ON CONFLICT (key) DO NOTHING;

-- ============ G2.2 授权继承（零漂移） ============
-- 凡当前持有 sitepacks.manage 的（档位/职务），同授三个细键，granted 原样复制
-- （含显式拒绝的行——保持一致，避免把「拒绝」洗成「未授」）。
INSERT INTO role_permissions (role_type, role_key, permission_key, granted)
SELECT rp.role_type, rp.role_key, k.key, rp.granted
FROM role_permissions rp
CROSS JOIN (VALUES ('custompages.manage'), ('userfields.manage'),
    ('terms.manage')) AS k(key)
WHERE rp.permission_key = 'sitepacks.manage'
ON CONFLICT DO NOTHING;

-- ============ G2.3 面板条目收口 ============
-- 三个条目此前 perm_key 为空（min_class 一过就可见，点进去动作却 403）：
-- 挂上细键后，面板可见性与端点权限同源（「只授页面管理」的职务能看见并只看见页面管理）。
UPDATE staff_panel_entries SET perm_key = 'custompages.manage'
WHERE tab_key = 'pages' AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'userfields.manage'
WHERE tab_key = 'userfields' AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'terms.manage'
WHERE tab_key = 'terms' AND perm_key IS NULL;

-- ============ G3.1 权限 → 模块归属 ============
ALTER TABLE permissions ADD COLUMN IF NOT EXISTS module_key text;

COMMENT ON COLUMN permissions.module_key IS
    '该权限所属模块键（modules 注册表）；非空时权限矩阵按模块开关过滤（modules::module_on_sql 口径：未配置键按关）';

-- 只登记「整个功能域由该模块开关决定」的权限；跨模块/核心件保持 NULL。
-- 归属依据 = staff_panel_entries 同域条目的 module_key（0196 口径），避免第二套词表。
UPDATE permissions SET module_key = 'shop'       WHERE key = 'prop.manage'      AND module_key IS NULL;
UPDATE permissions SET module_key = 'medals'     WHERE key = 'medal.manage'     AND module_key IS NULL;
UPDATE permissions SET module_key = 'exams'      WHERE key = 'exam.manage'      AND module_key IS NULL;
UPDATE permissions SET module_key = 'tasks'      WHERE key = 'task.manage'      AND module_key IS NULL;
UPDATE permissions SET module_key = 'attendance' WHERE key = 'attendance.manage' AND module_key IS NULL;
UPDATE permissions SET module_key = 'games'      WHERE key = 'fun.manage'       AND module_key IS NULL;
UPDATE permissions SET module_key = 'invites'    WHERE key = 'invite.view'      AND module_key IS NULL;
UPDATE permissions SET module_key = 'offers'     WHERE key = 'offers.promote'   AND module_key IS NULL;
UPDATE permissions SET module_key = 'forums'     WHERE key IN ('forums.manage', 'polls.manage') AND module_key IS NULL;
