-- 0034 站点设定对齐 NexusPHP settings.php 十三分组口径
-- 参考好学站（hxpt）settings.php 各 action 分组的全部配置键，
-- 现有 49 项之外补齐缺失键（键名保持 NexusPHP 惯例）。

-- 表本身没有 descr 列：先加（幂等）
ALTER TABLE site_settings ADD COLUMN IF NOT EXISTS descr text;
ALTER TABLE site_settings ADD COLUMN IF NOT EXISTS grp text;

-- ============ 基础设定 basicsettings ============
INSERT INTO site_settings (name, value, descr) VALUES
('SITENAME', '好学 FluxTorrent', '站点名称'),
('BASEURL', '127.0.0.1:3000', '站点根 URL'),
('announce_url', 'http://127.0.0.1:8080/announce', 'Tracker announce 地址')
ON CONFLICT (name) DO NOTHING;

-- ============ 主要设定 mainsettings ============
INSERT INTO site_settings (name, value, descr) VALUES
('iniupload', '1073741824', '新用户初始上传量（字节）'),
('invite_count', '5', '默认邀请名额'),
('invite_timeout', '604800', '邀请码有效期（秒）'),
('verification', 'none', '注册验证方式'),
('imdb_language', 'zh-CN', 'IMDb 信息语言'),
('max_torrent_size', '10737418240', '种子体积上限（字节）'),
('announce_interval', '1800', '客户端汇报间隔（秒）'),
('autoclean_interval_one', '3600', '自动清理间隔一（秒）'),
('signup_timeout', '259200', '注册超时（秒）'),
('minoffervotes', '10', '候选最少票数'),
('offervotetimeout', '2592000', '候选投票超时（秒）'),
('maxsubsize', '1048576', '字幕体积上限（字节）'),
('postsperpage', '10', '每页帖子数'),
('topicsperpage', '20', '每页主题数'),
('torrentsperpage', '50', '每页种子数'),
('maxnewsnum', '5', '首页公告条数'),
('maxusers', '50000', '用户数上限'),
('SITEEMAIL', 'noreply@flux.local', '站点邮箱'),
('PAYPALACCOUNT', '', 'PayPal 账户')
ON CONFLICT (name) DO NOTHING;

-- ============ SMTP 设定 smtpsettings ============
INSERT INTO site_settings (name, value, descr) VALUES
('smtp_host', '', 'SMTP 服务器'),
('smtp_port', '465', 'SMTP 端口'),
('smtp_from', '', '发件人地址'),
('encryption', 'ssl', '加密方式 ssl/tls/none'),
('accountname', '', 'SMTP 账号'),
('accountpassword', '', 'SMTP 密码')
ON CONFLICT (name) DO NOTHING;

-- ============ 安全设定 securitysettings ============
INSERT INTO site_settings (name, value, descr) VALUES
('securelogin', 'yes', '强制安全登录'),
('securetracker', 'yes', 'Tracker HTTPS'),
('cheaterdet', 'yes', '作弊者探测'),
('maxip', '3', '同 IP 可登录账号数'),
('maxloginattempts', '5', '失败登录锁定阈值'),
('loginattemptwhitelist', '', '登录白名单 IP'),
('guest_visit_type', 'redirect', '游客访问处理 static_page/redirect'),
('login_type', 'default', '登录方式'),
('login_secret_lifetime', '3600', '登录密钥有效期（秒）')
ON CONFLICT (name) DO NOTHING;

-- ============ 权限设定 authoritysettings（key = authority_<功能>，值 = 最低等级） ============
INSERT INTO site_settings (name, value, descr) VALUES
('authority_staffmem', '80', '群发管理组信箱最低等级'),
('authority_newsmanage', '60', '公告管理'),
('authority_funmanage', '60', '趣味盒管理'),
('authority_pollmanage', '60', '投票管理'),
('authority_applylink', '30', '申请友链'),
('authority_linkmanage', '80', '友链管理'),
('authority_postmanage', '50', '帖子管理'),
('authority_commanage', '50', '评论管理'),
('authority_forummanage', '80', '版块管理'),
('authority_torrentmanage', '80', '种子管理'),
('authority_torrentsticky', '80', '置顶种子'),
('authority_askreseed', '0', '求续种'),
('authority_viewnfo', '10', '查看 NFO'),
('authority_sendinvite', '30', '发送邀请'),
('authority_topten', '0', '查看排行'),
('authority_viewanonymous', '60', '查看匿名'),
('authority_beanonymous', '10', '匿名发布'),
('authority_addoffer', '10', '提交候选'),
('authority_offermanage', '80', '候选管理'),
('authority_upload', '30', '上传种子'),
('authority_movetorrent', '80', '移动种子分类'),
('authority_viewinvite', '30', '查看邀请记录'),
('authority_buyinvite', '30', '购买邀请'),
('authority_seebanned', '80', '查看封禁用户')
ON CONFLICT (name) DO NOTHING;

-- ============ 次要设定 tweaksettings ============
INSERT INTO site_settings (name, value, descr) VALUES
('titlekeywords', '教育,种子,PT', '标题关键词'),
('metakeywords', '教育,PT,种子', 'META 关键词'),
('metadescription', '教育资源私有种子社区', 'META 描述'),
('analyticscode', '', '统计代码'),
('datefounded', '2026-09-01', '建站日期')
ON CONFLICT (name) DO NOTHING;

-- ============ 火花设定 bonussettings ============
INSERT INTO site_settings (name, value, descr) VALUES
('donortimes', '2', '捐赠者火花倍数'),
('perseeding', '1', '每个做种基础火花'),
('maxseeding', '7', '计费做种上限'),
('tzero', '4', '公式 T0（小时）'),
('nzero', '7', '公式 N0（人数）'),
('zero_bonus_factor', '0.2', '零火花种子系数'),
('bzero', '100', '公式 B0'),
('l', '300', '公式 L'),
('uploadtorrent', '15', '发布种子奖励'),
('uploadsubtitle', '5', '发布字幕奖励'),
('starttopic', '2', '发主题奖励'),
('makepost', '1', '回帖奖励'),
('addcomment', '1', '评论奖励'),
('pollvote', '1', '投票奖励'),
('offervote', '1', '候选投票奖励'),
('funboxvote', '1', '趣味盒投票奖励'),
('saythanks', '0.5', '说谢谢奖励'),
('receivethanks', '0', '被感谢奖励'),
('funboxreward', '5', '趣味盒作品奖励'),
('prolinkpoint', '0', '推广链接奖励'),
('onegbupload', '300', '1GB 上传量价格'),
('fivegbupload', '800', '5GB 上传量价格'),
('tengbupload', '1200', '10GB 上传量价格'),
('hundredgbupload', '10000', '100GB 上传量价格'),
('tengbdownload', '1000', '10GB 下载量价格'),
('hundredgbdownload', '8000', '100GB 下载量价格'),
('oneinvite', '10000', '一个邀请价格'),
('customtitle', '5000', '自定义头衔价格'),
('basictax', '0', '交易基础税'),
('taxpercentage', '0', '交易税率（%）'),
('cancel_hr', '20000', '取消 H&R 价格'),
('attendance_card', '5000', '补签卡价格'),
('ratiolimit', '0.5', '低分享率限制'),
('dlamountlimit', '53687091200', '低下载量限制（字节）')
ON CONFLICT (name) DO NOTHING;

-- ============ 账号设定 accountsettings ============
INSERT INTO site_settings (name, value, descr) VALUES
('deletepacked', '30', '自动删号：停用账号（天）'),
('deleteunpacked', '60', '自动删号：未登录账号（天）'),
('deletenotransfer', '90', '无流量账号删号（天）'),
('destroy_disabled', 'no', '物理删除被禁账号'),
('deletepeasant', 'no', '删除低分享率用户'),
('psdlone', '10737418240', '降级线一：下载量'),
('psratioone', '0.4', '降级线一：分享率'),
('getInvitesByPromotion', 'yes', '升级赠邀请')
ON CONFLICT (name) DO NOTHING;

-- ============ 种子设定 torrentsettings ============
INSERT INTO site_settings (name, value, descr) VALUES
('randomfree', '0.02', '随机免费概率'),
('randomtwoup', '0.02', '随机 2x 概率'),
('randomhalfleech', '0.02', '随机 50% 概率'),
('randomtwoupfree', '0.01', '随机 2x免费 概率'),
('randomthirtypercentdown', '0.02', '随机 30% 概率'),
('largesize', '53687091200', '大种标准（字节）'),
('largepro', 'free', '大种自动促销'),
('freebecome', '86400', '免费时长（秒）'),
('expirefree', 'yes', '免费到期回落'),
('twoupbecome', '86400', '2x 时长（秒）'),
('normalbecome', '0', '促销种子回落普通'),
('claim_enabled', 'yes', '开启保种认领'),
('claim_torrent_ttl', '604800', '保种有效期（秒）'),
('claim_bonus_multiplier', '2', '保种火花倍数'),
('hotdays', '7', '热门天数'),
('hotseeder', '10', '热门做种数阈值'),
('uploaderdouble', 'yes', '发布者下载双倍'),
('deldeadtorrent', 'no', '自动删无种')
ON CONFLICT (name) DO NOTHING;

-- ============ 附件设定 attachmentsettings ============
INSERT INTO site_settings (name, value, descr) VALUES
('enableattach', 'no', '开启附件'),
('savedirectory', './attachments', '附件保存目录'),
('thumbnailtype', 'createthumb', '缩略图 no/createthumb/resizebigimg'),
('thumbquality', '80', '缩略图质量'),
('thumbwidth', '500', '缩略图宽'),
('thumbheight', '500', '缩略图高'),
('watermarkpos', 'no', '水印位置'),
('watermarkquality', '85', '水印 JPEG 质量')
ON CONFLICT (name) DO NOTHING;

-- ============ 广告设定 advertisementsettings ============
INSERT INTO site_settings (name, value, descr) VALUES
('enablead', 'yes', '开启广告'),
('enablenoad', 'yes', '允许用户免广告'),
('enablebonusnoad', 'yes', '允许火花购买免广告'),
('bonusnoadpoint', '10000', '免广告价格（火花）'),
('bonusnoadtime', '15', '免广告天数'),
('adclickbonus', '0', '点击广告奖励')
ON CONFLICT (name) DO NOTHING;

-- ============ 其他设定 miscsettings ============
INSERT INTO site_settings (name, value, descr) VALUES
('donation_custom', '', '捐赠说明'),
('protected_forum', '', '受保护版块 ID 列表')
ON CONFLICT (name) DO NOTHING;

-- ============ 分组元数据（前端十三分组渲染顺序） ============
INSERT INTO site_settings (name, value, descr) VALUES
('settings_group_order', 'basic,main,smtp,security,authority,tweak,bonus,account,torrent,attachment,advertisement,misc', '站点设定分组顺序')
ON CONFLICT (name) DO NOTHING;

-- 回填分组归属
UPDATE site_settings SET grp = 'basic' WHERE grp IS NULL AND name IN ('SITENAME','BASEURL','announce_url');
UPDATE site_settings SET grp = 'main' WHERE grp IS NULL AND name IN ('iniupload','invite_count','invite_timeout','verification','imdb_language','max_torrent_size','announce_interval','autoclean_interval_one','signup_timeout','minoffervotes','offervotetimeout','maxsubsize','postsperpage','topicsperpage','torrentsperpage','maxnewsnum','maxusers','SITEEMAIL','PAYPALACCOUNT');
UPDATE site_settings SET grp = 'smtp' WHERE grp IS NULL AND name IN ('smtp_host','smtp_port','smtp_from','encryption','accountname','accountpassword');
UPDATE site_settings SET grp = 'security' WHERE grp IS NULL AND name IN ('securelogin','securetracker','cheaterdet','maxip','maxloginattempts','loginattemptwhitelist','guest_visit_type','login_type','login_secret_lifetime');
UPDATE site_settings SET grp = 'authority' WHERE grp IS NULL AND name LIKE 'authority\_%';
UPDATE site_settings SET grp = 'tweak' WHERE grp IS NULL AND name IN ('titlekeywords','metakeywords','metadescription','analyticscode','datefounded');
UPDATE site_settings SET grp = 'bonus' WHERE grp IS NULL AND name IN ('donortimes','perseeding','maxseeding','tzero','nzero','zero_bonus_factor','bzero','l','uploadtorrent','uploadsubtitle','starttopic','makepost','addcomment','pollvote','offervote','funboxvote','saythanks','receivethanks','funboxreward','prolinkpoint','onegbupload','fivegbupload','tengbupload','hundredgbupload','tengbdownload','hundredgbdownload','oneinvite','customtitle','basictax','taxpercentage','cancel_hr','attendance_card','ratiolimit','dlamountlimit');
UPDATE site_settings SET grp = 'account' WHERE grp IS NULL AND name IN ('deletepacked','deleteunpacked','deletenotransfer','destroy_disabled','deletepeasant','psdlone','psratioone','getInvitesByPromotion');
UPDATE site_settings SET grp = 'torrent' WHERE grp IS NULL AND name IN ('randomfree','randomtwoup','randomhalfleech','randomtwoupfree','randomthirtypercentdown','largesize','largepro','freebecome','expirefree','twoupbecome','normalbecome','claim_enabled','claim_torrent_ttl','claim_bonus_multiplier','hotdays','hotseeder','uploaderdouble','deldeadtorrent');
UPDATE site_settings SET grp = 'attachment' WHERE grp IS NULL AND name IN ('enableattach','savedirectory','thumbnailtype','thumbquality','thumbwidth','thumbheight','watermarkpos','watermarkquality');
UPDATE site_settings SET grp = 'advertisement' WHERE grp IS NULL AND name IN ('enablead','enablenoad','enablebonusnoad','bonusnoadpoint','bonusnoadtime','adclickbonus');
UPDATE site_settings SET grp = 'misc' WHERE grp IS NULL AND name IN ('donation_custom','protected_forum','settings_group_order');
-- 既有 49 项归入最接近的分组
UPDATE site_settings SET grp = 'basic' WHERE grp IS NULL AND name IN ('site_name','site_title','site_subtitle','timezone','default_language','stylesheet_default');
UPDATE site_settings SET grp = 'security' WHERE grp IS NULL AND name IN ('register_open','enable_two_step','enable_passkey_login');
UPDATE site_settings SET grp = 'bonus' WHERE grp IS NULL AND name IN ('enable_bonus','bonus_per_seed','attendance_bonus','enable_attendance','freeleech_until','noad_hours');
UPDATE site_settings SET grp = 'torrent' WHERE grp IS NULL AND name IN ('enable_torrent_upload','torrent_autoswitch','max_torrent_size','hr_hours','enable_hr');
UPDATE site_settings SET grp = 'main' WHERE grp IS NULL AND (name LIKE 'enable\_%' OR name LIKE '%per_page%' OR name LIKE '%_default');
