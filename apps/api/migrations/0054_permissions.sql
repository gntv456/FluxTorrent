-- 权限底座：permissions（权限项）+ roles（职务定义）+ role_permissions（角色→权限）+ user_roles（用户→职务）
--
-- 背景：原权限判定是 48 处散落的 class_id 字面量阈值（只有 90/93/99 三档），
--       改权限需动代码重新构建，且无法表达「发布员」这类无高低的职能职务。
-- 方案：统一为 user_can(perm) 入口，权限来源取三者并集：
--       ① class 累进（class_id >= role_key）　② 职务（user_roles 中存在对应 role_key）
--       等级锚点仍用 90/93/99，但改为数据行，后台可调。

-- ============ 权限项清单 ============
CREATE TABLE IF NOT EXISTS permissions (
    key      text PRIMARY KEY,
    name     text NOT NULL,
    category text NOT NULL,
    descr    text,
    sort     integer NOT NULL DEFAULT 0
);

-- ============ 职务定义（可兼任，无高低） ============
CREATE TABLE IF NOT EXISTS roles (
    key    text PRIMARY KEY,
    name   text NOT NULL,
    descr  text,
    sort   integer NOT NULL DEFAULT 0
);

-- ============ 角色 → 权限映射 ============
-- role_type='class'：role_key 为等级数字，判定为 class_id >= role_key（累进）
-- role_type='role' ：role_key 为 roles.key，判定为用户持该职务
CREATE TABLE IF NOT EXISTS role_permissions (
    role_type      text NOT NULL CHECK (role_type IN ('class','role')),
    role_key       text NOT NULL,
    permission_key text NOT NULL REFERENCES permissions(key) ON DELETE CASCADE,
    granted        boolean NOT NULL DEFAULT true,
    PRIMARY KEY (role_type, role_key, permission_key)
);
CREATE INDEX IF NOT EXISTS idx_role_perms_lookup ON role_permissions (permission_key, role_type, role_key);

-- ============ 用户 → 职务（多对多，可兼任） ============
CREATE TABLE IF NOT EXISTS user_roles (
    user_id    bigint NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_key   text NOT NULL REFERENCES roles(key) ON DELETE CASCADE,
    granted_by bigint,
    granted_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz,
    PRIMARY KEY (user_id, role_key)
);
CREATE INDEX IF NOT EXISTS idx_user_roles_user ON user_roles (user_id);

-- ============ 种子数据：权限项 ============
DELETE FROM permissions;
INSERT INTO permissions (key, name, category, descr, sort) VALUES
-- 发布（upload）
('torrent.upload',            '发布种子',       'upload',  '上传新种子',                          10),
('torrent.approval.auto',     '发种免审核',     'upload',  '发布即通过，无需人工审核',            11),
('torrent.set_price',         '设置促销价格',   'upload',  '为种子设置免费/双倍等促销状态',       12),
('torrent.view_anonymous',    '查看匿名发布者', 'upload',  '可见匿名种子的真实发布者',            13),
('torrent.upload_special',    '发布特殊资源',   'upload',  '可发布受限类别资源',                  14),
('torrent.see_banned',        '查看被禁资源',   'upload',  '可见已被封禁的种子',                  15),
('torrent.repost',            '转载资源',       'repost',  '从外部站批量转载入站',                16),
-- 保种（seed）
('hr.exempt',                 '免除 H&R',       'seed',    '不受下载后做种时长要求约束',          20),
('seed.stats.view',           '保种统计',       'seed',    '查看站点保种与做种分布统计',          21),
-- 外联（liaison）
('invites.bonus',             '邀请配额加成',   'liaison', '邀请配额提高',                        30),
('announce.publish',          '发布站点公告',   'liaison', '代表站点发布对外公告',                31),
-- 内容（content）
('faq.manage',                'FAQ 管理',       'content', '常见问题增删改',                      40),
('rules.manage',              '规则管理',       'content', '站点规则增删改',                      41),
('news.manage',               '公告管理',       'content', '首页公告增删改',                      42),
('links.manage',              '友情链接',       'content', '友链增删改',                          43),
('fun.manage',                '娱乐条目',       'content', '娱乐板块条目增删改',                  44),
('ads.manage',                '广告管理',       'content', '站点广告增删改',                      45),
('categories.manage',         '分类管理',       'content', '种子分类模式增删改',                  46),
('forums.manage',             '论坛版块',       'content', '论坛版块与版主任免',                  47),
('polls.manage',              '投票管理',       'content', '站点投票管理',                        48),
('offers.promote',            '候选转正',       'content', '将求种候选转正为官方资源',            49),
('agents.view',               '客户端会话',     'system',  '查看全部客户端当前会话',             110),
('notconnectable.view',       '无法连接用户',   'system',  '查看无法连接的用户列表',             111),
-- 用户（user）
('user.warn',                 '警告用户',       'user',    '发出/解除用户警告',                   60),
('user.status',               '封禁/解封',      'user',    '变更用户账号状态',                    61),
('user.flags',                '挂起/禁下载',    'user',    '切换用户挂起与下载开关',              62),
('user.adjust',               '数值调整',       'user',    '增减上传量/下载量/魔力',              63),
('user.class',                '调整等级',       'user',    '变更用户等级',                        64),
('user.create',               '添加用户',       'user',    '直接创建新账号',                      65),
('user.resetpass',            '重置密码',       'user',    '重置他人密码',                        66),
('user.delete_disabled',      '删除被禁用户',   'user',    '批量删除已禁用账号',                  67),
('user.amountbonus',          '增减魔力',       'user',    '为特定等级批量增减魔力',              68),
('user.amountupload',         '增加上传量',     'user',    '为特定等级批量增加上传量',            69),
('appeal.handle',             '申诉处理',       'user',    '处理封禁与警告申诉',                  70),
('ip.check',                  '重复 IP 检测',   'user',    '查看相同 IP 用户',                    71),
('uploaders.view',            '上传者状态',     'user',    '查看上传者列表',                      72),
('hr.pardon',                 'H&R 赦免',       'user',    '赦免命中下载限制的用户',              73),
('staff.panel',               '管理面板',       'user',    '访问管理后台',                        79),
('staff.message',             '管理组信箱',     'user',    '收发管理组站内信',                    80),
-- 运营（site）
('staffmess',                 '群发站内信',     'site',    '向全部用户发送站内私信',              90),
('massmail',                  '群发邮件',       'site',    '向全部用户发送邮件',                  91),
('emailban.manage',           '邮件黑白名单',   'site',    '管理注册邮件域名黑白名单',            92),
('freeleech.view',            '查看促销',       'site',    '查看进行中的促销活动',                93),
('freeleech.manage',          '管理促销',       'site',    '创建/取消促销活动',                   94),
('settings.view',             '查看站点设定',   'site',    '查看站点配置与修改历史',              94),
('settings.manage',           '站点设定',       'site',    '修改站点配置',                        95),
('locations.manage',          '位置管理',       'site',    '管理地址及地址速度',                  96),
('sitepacks.manage',          '类型包管理',     'site',    '站点类型包应用',                      97),
-- 系统（system）
('audit.view',                '审计日志',       'system',  '查看管理操作记录',                   100),
('syslog.view',               '系统日志',       'system',  '查看系统运行日志',                   101),
('dbstats.view',              '数据库状态',     'system',  '查看数据库运行状态',                 102),
('stats.view',                '站点统计',       'system',  '查看服务器相关统计',                 103),
('cleanup.run',               '运行清理',       'system',  '手动触发清理函数',                   104),
('clearcache',                '清除缓存',       'system',  '清除运行期缓存键',                   105),
('bans.manage',               'IP 封禁',        'system',  '禁止/取消禁止 IP',                   106),
('testip',                    'IP 测试',        'system',  '测试 IP 是否被禁止',                 107),
('maxlogin.view',             '失败登录',       'system',  '查看失败登录尝试',                   108),
('plugins.manage',            '插件管理',       'system',  '管理站点插件',                       109);

-- ============ 种子数据：职务 ============
DELETE FROM roles;
INSERT INTO roles (key, name, descr, sort) VALUES
('uploader',  '发布员', '负责资源发布，发种免审核',       10),
('reposter',  '转载员', '负责从外部站转载资源',           11),
('seeder',    '保种员', '负责长期保种，免除 H&R 要求',     12),
('liaison',   '外联员', '负责对外联络、友链与公告',       13),
('forum_mod', '论坛版主', '负责论坛版块与帖务管理',       14);

-- ============ 种子数据：角色 → 权限 ============
DELETE FROM role_permissions;
-- 基础层：所有注册用户
INSERT INTO role_permissions (role_type, role_key, permission_key)
SELECT 'class', '1', key FROM permissions WHERE key IN ('torrent.upload');

-- 贵宾 VIP（身份层 class_id=20）
INSERT INTO role_permissions (role_type, role_key, permission_key)
SELECT 'class', '20', key FROM permissions WHERE key IN ('hr.exempt','invites.bonus','seed.stats.view');

-- 版主（class >= 90）：内容管理 + 用户监察 + 站点工具
INSERT INTO role_permissions (role_type, role_key, permission_key)
SELECT 'class', '90', key FROM permissions WHERE key IN (
  'staff.panel','staff.message','faq.manage','rules.manage','news.manage','links.manage',
  'fun.manage','ads.manage','polls.manage','user.warn','user.status','user.flags',
  'appeal.handle','ip.check','uploaders.view','hr.pardon','freeleech.view','audit.view',
  'stats.view','clearcache','bans.manage','testip','plugins.manage',
  'staffmess','agents.view','notconnectable.view','settings.view','offers.promote');

-- 总版主（class >= 93）：增加论坛版块与数值调整
INSERT INTO role_permissions (role_type, role_key, permission_key)
SELECT 'class', '93', key FROM permissions WHERE key IN ('forums.manage','user.adjust');

-- 站长（class >= 99）：高危操作全量
-- 注：staffmess（群发站内信）原守卫为 <90 即 90+，故归 90 档而非此处，避免改造后收窄权限
INSERT INTO role_permissions (role_type, role_key, permission_key)
SELECT 'class', '99', key FROM permissions WHERE key IN (
  'categories.manage','user.class','user.create','user.resetpass','user.delete_disabled',
  'user.amountbonus','user.amountupload','massmail','emailban.manage',
  'freeleech.manage','settings.manage','locations.manage','sitepacks.manage',
  'syslog.view','dbstats.view','cleanup.run','maxlogin.view');

-- 职务：发布员
INSERT INTO role_permissions (role_type, role_key, permission_key)
SELECT 'role', 'uploader', key FROM permissions WHERE key IN (
  'torrent.approval.auto','torrent.set_price','torrent.view_anonymous',
  'torrent.upload_special','torrent.see_banned');

-- 职务：转载员
INSERT INTO role_permissions (role_type, role_key, permission_key)
SELECT 'role', 'reposter', key FROM permissions WHERE key IN ('torrent.repost','torrent.upload_special');

-- 职务：保种员
INSERT INTO role_permissions (role_type, role_key, permission_key)
SELECT 'role', 'seeder', key FROM permissions WHERE key IN ('hr.exempt','seed.stats.view');

-- 职务：外联员
INSERT INTO role_permissions (role_type, role_key, permission_key)
SELECT 'role', 'liaison', key FROM permissions WHERE key IN ('links.manage','invites.bonus','announce.publish');

-- 职务：论坛版主
INSERT INTO role_permissions (role_type, role_key, permission_key)
SELECT 'role', 'forum_mod', key FROM permissions WHERE key IN ('forums.manage');

-- ============ 导航登记：职务管理页签 ============
INSERT INTO staff_panel_entries (panel, name, url, info, sort, section, min_class, tab_key) VALUES
('admin', '职务管理', '/admin?tool=roles', '授予或撤销职能职务（可兼任）', 11, 'users', 93, 'roles')
ON CONFLICT DO NOTHING;
