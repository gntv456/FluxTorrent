-- 管理组面板信息架构重构：权限桶分组 → 职能分组 + 细粒度等级过滤
--
-- 背景：staff_panel_entries.panel 原本是权限等级（sysop/admin/moderator），被当作分组维度使用，
--       导致同一职能的条目被拆散到三个桶里；且它与前端硬编码的 staff-tools 32 个页签是两套
--       重复入口、命名不一致，另有 25 个 /admin?tool= 值无对应实现（死链）。
-- 方案：panel 保留做兼容但不再用于分组；新增 section（职能分组）/ min_class（0-99 细粒度）
--       / tab_key（指向具体工具组件，保证每个 URL 都有确定归属）。

ALTER TABLE staff_panel_entries
  ADD COLUMN IF NOT EXISTS section text NOT NULL DEFAULT 'system',
  ADD COLUMN IF NOT EXISTS min_class integer NOT NULL DEFAULT 90,
  ADD COLUMN IF NOT EXISTS tab_key text NOT NULL DEFAULT '';

CREATE INDEX IF NOT EXISTS idx_staff_panel_section ON staff_panel_entries (section, sort);
CREATE INDEX IF NOT EXISTS idx_staff_panel_min_class ON staff_panel_entries (min_class);

-- 全量重建 44 条：合并两套入口、统一命名、补齐此前只在前端硬编码的功能
DELETE FROM staff_panel_entries;

INSERT INTO staff_panel_entries (panel, name, url, info, sort, section, min_class, tab_key) VALUES
-- 工作台
('moderator', '工作台',       '/admin?tool=overview',    '站点概览与待办数字',            0,  'dashboard',  90, 'overview'),
-- 审核队列
('moderator', '种子审核',     '/admin?tool=reviews',     '审核新发布的种子',              0,  'moderation', 90, 'reviews'),
('moderator', '举报处理',     '/admin?tool=reports',     '处理用户举报',                  1,  'moderation', 90, 'reports'),
('moderator', '用户申诉',     '/admin?tool=appeals',     '处理封禁与警告申诉',            2,  'moderation', 90, 'appeals'),
('moderator', '作弊探测',     '/admin?tool=cheaters',    '异常上传速度检测',              3,  'moderation', 90, 'cheaters'),
-- 用户
('moderator', '用户查询',     '/admin?tool=users',       '搜索用户并调整等级与状态',      0,  'users',      90, 'users'),
('admin',     '添加用户',     '/admin?tool=adduser',     '添加新的用户账号',              1,  'users',      93, 'adduser'),
('admin',     '重置密码',     '/admin?tool=resetpass',   '重置丢失的密码',                2,  'users',      93, 'resetpass'),
('moderator', '魔力增减',     '/admin?tool=bonus',       '为某个或全部用户增减魔力',      3,  'users',      90, 'bonus'),
('sysop',     '上传量增减',   '/admin?tool=amountupload','为特定等级用户增加上传量',      4,  'users',      99, 'upload'),
('admin',     '警告用户',     '/admin?tool=warned',      '查看全部被警告的用户',          5,  'users',      93, 'warned'),
('sysop',     '删除被禁用户', '/admin?tool=deldisabled', '删除所有被禁用的用户',          6,  'users',      99, 'deldisabled'),
('moderator', '重复 IP 检测', '/admin?tool=ipcheck',     '查看相同 IP 的用户',            7,  'users',      90, 'ipcheck'),
('admin',     '无法连接的用户','/admin?tool=notconnect', '查看全部无法连接的用户',        8,  'users',      93, 'notconnect'),
('moderator', '上传者',       '/admin?tool=uploaders',   '查看上传者状态',                9,  'users',      90, 'uploaders'),
('moderator', 'HR 赦免',      '/admin?tool=hrpardon',    '赦免命中下载限制的用户',        10, 'users',      90, 'hrpardon'),
-- 内容
('moderator', '种子管理',     '/admin?tool=torrents',    '搜索并管理站点种子',            0,  'content',    90, 'torrents'),
('admin',     '分类管理',     '/admin?tool=cats',        '管理种子分类模式',              1,  'content',    93, 'cats'),
('admin',     '常见问题',     '/admin?tool=faq',         '编辑/增加/删除 常见问题',       2,  'content',    93, 'faq'),
('admin',     '规则管理',     '/admin?tool=rules',       '编辑/增加/删除 规则',           3,  'content',    93, 'rules'),
('sysop',     '论坛版块',     '/admin?tool=forums',      '编辑/删除论坛版块',             4,  'content',    99, 'forums'),
('admin',     '投票',         '/admin?tool=polls',       '查看全部投票',                  5,  'content',    93, 'polls'),
('admin',     '首页内容',     '/admin?tool=content',     '首页公告、娱乐与友链',          6,  'content',    93, 'content'),
('moderator', '广告',         '/admin?tool=ads',         '管理网站上的广告',              7,  'content',    90, 'ads'),
('admin',     '导航菜单',     '/admin?tool=menu',        '自定义顶栏导航与等级门槛',      8,  'content',    93, 'menu'),
-- 运营
('sysop',     '站点设定',     '/admin/settings',         '类型化站点配置与修改历史',      0,  'ops',        99, 'settings'),
('admin',     '促销公告',     '/admin?tool=promo',       '首页公告条与置顶促销',          1,  'ops',        93, 'promo'),
('admin',     '批量私信',     '/admin?tool=staffmess',   '发送站内私信给全部用户',        2,  'ops',        93, 'staffmess'),
('sysop',     '邮件群发',     '/admin?tool=massmail',    '发送邮件给全部用户',            3,  'ops',        99, 'mail'),
('sysop',     '邮件黑白名单', '/admin?tool=emailbans',   '禁止或允许邮件地址注册',        4,  'ops',        99, 'emailbans'),
('admin',     '免费/促销状态','/admin?tool=freeleech',   '设定全部种子为某种促销状态',    5,  'ops',        93, 'freeleech'),
('admin',     '运营配置',     '/admin?tool=p2tools',     '公告条、消息模板与认领管理',    6,  'ops',        93, 'p2tools'),
-- 系统
('moderator', '审计日志',     '/admin?tool=audit',       '查看管理操作记录',              0,  'system',     90, 'audit'),
('sysop',     '系统日志',     '/admin?tool=syslog',      '查看系统运行日志',              1,  'system',     99, 'syslog'),
('sysop',     '数据库状态',   '/admin?tool=dbstats',     '查看数据库运行状态',            2,  'system',     99, 'dbstats'),
('moderator', '统计',         '/admin?tool=stats',       '服务器相关数据统计',            3,  'system',     90, 'stats'),
('sysop',     '清理',         '/admin?tool=cleanup',     '运行清理函数',                  4,  'system',     99, 'cleanup'),
('moderator', '清除缓存',     '/admin?tool=clearcache',  '清除缓存的数据',                5,  'system',     90, 'clearcache'),
('sysop',     '禁止 IP',      '/admin?tool=bans',        '禁止/取消禁止 IP',              6,  'system',     99, 'bans'),
('moderator', 'IP 测试',      '/admin?tool=testip',      '测试 IP 是否被禁止',            7,  'system',     90, 'testip'),
('sysop',     '失败登录',     '/admin?tool=maxlogin',    '查看失败的登录尝试',            8,  'system',     99, 'maxlogin'),
('moderator', '全部客户端',   '/admin?tool=agents',      '查看全部客户端（当前）',        9,  'system',     90, 'agents'),
('sysop',     '客户端名单',   '/admin?tool=agentrules',  '管理允许的客户端白名单',        10, 'system',     99, 'agentrules'),
('sysop',     '位置',         '/admin?tool=locations',   '管理地址及地址速度',            11, 'system',     99, 'locations'),
('sysop',     '插件',         '/admin?tool=plugins',     '管理站点插件',                  12, 'system',     99, 'plugins');
