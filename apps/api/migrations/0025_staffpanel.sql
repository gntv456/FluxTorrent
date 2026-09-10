-- 0025: 管理组面板（staffpanel 三级权限入口表）+ 站点设定（sysop 读写键值）
-- 复刻 NexusPHP staffpanel.php：SysOp / Administrator / Moderator 三组入口，
-- 每条含 name（i18n 键）+ url + info（i18n 键）。站点管理员（99）另见站点设定/管理系统。
CREATE TABLE IF NOT EXISTS staff_panel_entries (
  id SERIAL PRIMARY KEY,
  panel TEXT NOT NULL CHECK (panel IN ('sysop','admin','moderator')),
  name TEXT NOT NULL,             -- 显示名（中文）
  url TEXT NOT NULL,              -- 站内入口路径
  info TEXT NOT NULL,             -- 说明
  sort INT NOT NULL DEFAULT 0
);

-- 幂等种子：与 NexusPHP dbstructure.sql 的 sysoppanel/adminpanel/modpanel 一致
INSERT INTO staff_panel_entries (panel, name, url, info, sort)
SELECT * FROM (VALUES
  -- SysOp（对系统管理员开放）
  ('sysop','删除被禁用户','/admin?tool=deletedisabled','删除所有被禁用的用户',1),
  ('sysop','管理论坛','/forums','编辑/删除论坛版块',2),
  ('sysop','Mysql 状态','/admin?tool=mysql_stats','查看 Mysql 状态',3),
  ('sysop','批量邮件','/admin?tool=massmail','发送邮件给全部用户',4),
  ('sysop','做清理','/admin?tool=docleanup','运行清理函数',5),
  ('sysop','禁止系统','/admin?tool=bans','禁止/取消禁止 IP',6),
  ('sysop','失败登录','/admin?tool=maxlogin','查看失败的登录尝试',7),
  ('sysop','Bitbucket Log','/admin?tool=bitbucketlog','Bitbucket Log',8),
  ('sysop','禁止邮件地址','/admin?tool=bannedemails','禁止邮件地址注册',9),
  ('sysop','允许邮件地址','/admin?tool=allowedemails','允许邮件地址注册',10),
  ('sysop','位置','/admin?tool=location','管理地址及地址速度',11),
  ('sysop','增加上传','/admin?tool=amountupload','为特定等级用户增加上传量',12),
  -- Administrator（对管理员开放）
  ('admin','添加用户','/admin?tool=adduser','添加新的用户账号',1),
  ('admin','重置用户密码','/admin?tool=reset','重置丢失的密码',2),
  ('admin','批量私信','/admin?tool=staffmess','发送站内私信给全部用户',3),
  ('admin','无法连接的用户','/admin?tool=notconnectable','查看全部无法连接的用户',4),
  ('admin','投票','/admin?tool=polloverview','查看全部投票',5),
  ('admin','警告用户','/admin?tool=warned','查看全部被警告的用户',6),
  ('admin','免费下载','/admin?tool=freeleech','设定全部种子为某种状态',7),
  ('admin','常见问题管理','/admin?tool=faqmanage','编辑/增加/删除 常见问题',8),
  ('admin','规则管理','/admin?tool=modrules','编辑/增加/删除 规则',9),
  ('admin','分类管理','/admin?tool=catmanage','管理种子分类模式',10),
  -- Moderator（针对版主开放）
  ('moderator','异常上传速度探测','/admin?tool=cheaters','查看作弊者',1),
  ('moderator','重复 IP 检测','/admin?tool=ipcheck','查看相同 IP 用户',2),
  ('moderator','全部客户端（当前）','/admin?tool=allagents','查看全部客户端（当前下载中/上传中/做种中）',3),
  ('moderator','广告管理','/admin?tool=admanage','管理网站上的广告',4),
  ('moderator','上传者','/admin?tool=uploaders','查看上传者状态',5),
  ('moderator','统计','/admin?tool=stats','服务器相关数据统计',6),
  ('moderator','IP 测试','/admin?tool=testip','测试 IP 是否被禁止',7),
  ('moderator','增加魔力','/admin?tool=amountbonus','为某个或全部用户增加魔力',8),
  ('moderator','清除缓存','/admin?tool=clearcache','清除缓存的数据',9)
) AS seed(panel, name, url, info, sort)
WHERE NOT EXISTS (SELECT 1 FROM staff_panel_entries);

-- 站点设定（对齐 NexusPHP settings 表：sysop 可编辑的键值）
CREATE TABLE IF NOT EXISTS site_settings (
  name TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 幂等种子：常用站点设定默认值（包子站口径）
INSERT INTO site_settings (name, value)
SELECT * FROM (VALUES
  ('site_name','包子PT'),
  ('site_title','baozi'),
  ('site_subtitle','Share with love, Stay cozy'),
  ('announcement','欢迎来到包子PT'),
  ('enable_invite','yes'),
  ('invite_quota_user','1'),
  ('register_open','no'),
  ('enable_torrent_upload','yes'),
  ('torrent_autoswitch','yes'),
  ('enable_freeleech','no'),
  ('freeleech_until',''),
  ('enable_two_step','yes'),
  ('enable_passkey_login','yes'),
  ('max_torrent_size','21474836480'),
  ('enable_hr','yes'),
  ('hr_hours','120'),
  ('enable_bonus','yes'),
  ('bonus_per_seed','0.4'),
  ('enable_attendance','yes'),
  ('attendance_bonus','200'),
  ('enable_ad','yes'),
  ('noad_hours','12'),
  ('enable_adult','no'),
  ('enable_forum','yes'),
  ('enable_offers','yes'),
  ('enable_subtitle','yes'),
  ('enable_medal','yes'),
  ('enable_task','yes'),
  ('enable_bank','yes'),
  ('enable_games','yes'),
  ('enable_farm','yes'),
  ('enable_dressup','yes'),
  ('enable_shoutbox','yes'),
  ('enable_searchbox','yes'),
  ('enable_lastfm','no'),
  ('enable_wiki','no'),
  ('enable_uploadtop','yes'),
  ('enable_gzip','yes'),
  ('cache_driver','redis'),
  ('timezone','Asia/Shanghai'),
  ('default_language','chs'),
  ('stylesheet_default','BaoziPT'),
  ('torrents_per_page_default','50'),
  ('topics_per_page_default','20'),
  ('posts_per_page_default','10'),
  ('pm_per_page_default','10'),
  ('max_pm_per_page','100'),
  ('user_delete_days','40'),
  ('peasant_warn_days','30')
) AS seed(name, value)
WHERE NOT EXISTS (SELECT 1 FROM site_settings);
