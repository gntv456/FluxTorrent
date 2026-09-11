-- 0039 站点设定类型化元数据（settings_meta）—— 方案 P0
-- 依据：FluxTorrent站点设定复刻与UI重设计方案 §3（数据模型）、§3.2（类型系统）、§9（字段映射表）
-- 目标：用「配置声明」驱动渲染 / 校验 / 权限三件事：后端与前端共享同一份 schema，
--       英文键名不再裸露（label_zh / label_en 落库，前端 i18n 覆盖）。
--
-- 本迁移做四件事：
--   1) 建 settings_meta 表（规格见 §3.1；另加两列扩展：min_class 见 §7.1、visible 用于隐藏元数据键）；
--   2) 按 §9 红字清单补齐 site_settings 缺口键（ON CONFLICT DO NOTHING 幂等）；
--   3) 为全部 site_settings 键播种 meta（显式声明 + 兜底声明，保证零裸键）；
--   4) 补关键字段的帮助说明（hint）。
--
-- 写法约定：按「列型」分块 INSERT，每块列数严格一致，避免列数错位。

-- ============================================================================
-- 1) settings_meta 表
-- ============================================================================
CREATE TABLE IF NOT EXISTS settings_meta (
    name        text PRIMARY KEY REFERENCES site_settings(name) ON DELETE CASCADE,
    -- text|number|yesno|enum|password|textarea|color|classlevel|pair
    type        text NOT NULL DEFAULT 'text',
    label_zh    text,                                   -- 中文名（渲染主标签，不再回退英文键名）
    label_en    text,                                   -- 英文名（en / zh-TW 语言包覆盖）
    hint        text,                                   -- 帮助说明
    unit        text,                                   -- 单位：字节 / 秒 / % / 天 / 个 / 火花 / 小时
    min         double precision,                       -- 数值下限
    max         double precision,                       -- 数值上限
    step        double precision DEFAULT 1,             -- 数值步长（=1 视为整数）
    options     jsonb,                                  -- enum 选项 / 文本校验规则
    secret      boolean DEFAULT false,                  -- 密文字段：GET 回掩码，留空保持不变
    readonly    boolean DEFAULT false,                  -- 只读信息（版本号等）
    group_key   text,                                   -- 分区内卡片分组
    card_order  int,                                    -- 卡片内排序
    min_class   int DEFAULT 99,                         -- 【扩展】可写该字段的最低等级（§7.1 预留位）
    visible     boolean DEFAULT true,                   -- 【扩展】false = 不参与表单渲染（元数据键）
    updated_at  timestamptz DEFAULT now()
);

-- ============================================================================
-- 2) 补齐 §9 红字缺口键（site_settings）
-- ============================================================================
INSERT INTO site_settings (name, value, descr, grp) VALUES
-- 主要设定
('tmp_invite_count', '0', '临时邀请名额', 'main'),
('SLOGAN', '', '站点标语', 'main'),
('reportemail', '', '举报处理邮箱', 'main'),
('icplicense', '', 'ICP 备案号', 'main'),
('extforumurl', '', '外部论坛地址', 'main'),
('torrent_dir', './torrents', '种子文件存放目录', 'main'),
('torrentnameprefix', '', '种子文件名前缀', 'main'),
('max_dead_torrent_time', '2592000', '死种判定时长（秒）', 'main'),
('bitbucket', 'default', '官方发布小组标识', 'main'),
('annintertwo', '86400', '公告二显示时长（秒）', 'main'),
('anninterthree', '259200', '公告三显示时长（秒）', 'main'),
('annintertwoage', '30', '公告二提前量（天）', 'main'),
('anninterthreeage', '90', '公告三提前量（天）', 'main'),
('specialcat', '', '特殊分类 ID（逗号分隔）', 'main'),
('browsecat', '', '浏览分类 ID（逗号分隔）', 'main'),
('defaultlang', 'zh-CN', '默认语言', 'main'),
-- 模块开关：运行时由「站点类型切换」写入，此处补种保证全新库可迁移
('module_textbooks', 'yes', '启用课本中心', 'main'),
('module_showcase', 'no', '启用展示区', 'main'),
('site_language_enabled', 'yes', '开启多语言切换', 'main'),
('logo', '/logo.png', '站点 Logo 地址', 'main'),
('carousel_images', '', '首页轮播图（每行一个 URL）', 'main'),
('nfo_view_style_default', '0', 'NFO 查看样式默认值', 'main'),
('offeruptimeout', '2592000', '候选上架超时（秒）', 'main'),
('offer_skip_approved_count', '3', '候选自动转正所需赞数', 'main'),
('upload_deny_approval_deny_count', '3', '被拒阈值（触发禁止上传）', 'main'),
-- SMTP 设定
('smtpaddress', '', 'SMTP 发件显示地址', 'smtp'),
('smtpport', '465', 'SMTP 备用端口', 'smtp'),
-- 安全设定
('https_announce_url', '', 'HTTPS announce 地址', 'security'),
('enable_attendance_captcha', 'no', '签到启用验证码', 'security'),
('enable_image_verification', 'no', '启用图形验证码', 'security'),
('cheater_detection_level', 'normal', '作弊探测等级', 'security'),
('use_challenge_response_authentication', 'no', '启用质询响应认证', 'security'),
-- 权限设定（补齐至 NP 全量口径）
('defaultclass', '0', '新用户默认等级', 'authority'),
('staffmem', '80', '站务最低等级', 'authority'),
('authority_visit', '0', '访问站点最低等级', 'authority'),
('authority_download', '0', '下载种子最低等级', 'authority'),
('authority_addcomment', '0', '发表评论最低等级', 'authority'),
('authority_uploadsubtitle', '10', '上传字幕最低等级', 'authority'),
('authority_seeuser', '0', '查看用户最低等级', 'authority'),
('authority_viewuserlist', '0', '查看用户列表最低等级', 'authority'),
('authority_viewuseremail', '80', '查看用户邮箱最低等级', 'authority'),
('authority_viewuserip', '80', '查看用户 IP 最低等级', 'authority'),
('authority_viewuserhistory', '80', '查看用户历史最低等级', 'authority'),
('authority_viewtorrenthistory', '80', '查看种子历史最低等级', 'authority'),
('authority_viewstats', '0', '查看站点统计最低等级', 'authority'),
('authority_viewpeers', '80', '查看 peer 列表最低等级', 'authority'),
('authority_viewfunbox', '0', '查看趣味盒最低等级', 'authority'),
('authority_viewpoll', '0', '查看投票最低等级', 'authority'),
('authority_viewofferlist', '0', '查看候选列表最低等级', 'authority'),
('authority_offervote', '10', '候选投票最低等级', 'authority'),
('authority_pollvote', '10', '投票参与最低等级', 'authority'),
('authority_funboxvote', '10', '趣味盒投票最低等级', 'authority'),
('authority_seelog', '80', '查看日志最低等级', 'authority'),
('authority_seetorrentstructure', '80', '查看种子结构最低等级', 'authority'),
('authority_chat', '0', '聊天室发言最低等级', 'authority'),
('authority_editstaffbox', '80', '编辑管理组信箱最低等级', 'authority'),
('authority_seecheater', '80', '查看作弊者列表最低等级', 'authority'),
-- 次要设定
('cssdate', '', 'CSS 版本日期', 'tweak'),
-- 火花设定
('prolinktime', '0', '推广链接有效期（秒）', 'bonus'),
('vipstatus', 'no', 'VIP 身份标识', 'bonus'),
('official_addition', '0', '官方发布额外加成（%）', 'bonus'),
('harem_addition', '0', '后宫额外加成（%）', 'bonus'),
('one_tmp_invite', '5000', '临时邀请价格（火花）', 'bonus'),
('change_username_card', '10000', '改名卡价格（火花）', 'bonus'),
('rainbow_id', '10000', '彩虹 ID 价格（火花）', 'bonus'),
('attendance_initial', '10', '签到基础奖励（火花）', 'bonus'),
('attendance_step', '5', '连签每日递增（火花）', 'bonus'),
('attendance_max', '1000', '签到单日封顶（火花）', 'bonus'),
('attendance_continuous_day', '10,20,30', '连签里程碑天数（逗号分隔）', 'bonus'),
('attendance_continuous_value', '200,500,1000', '连签里程碑奖励（逗号分隔）', 'bonus'),
-- 账号设定
('deletenotransfertwo', '120', '无流量账号二次删号（天）', 'account'),
('psdltwo', '107374182400', '降级线二：下载量', 'account'),
('psdlthree', '1099511627776', '降级线三：下载量', 'account'),
('psdlfour', '0', '降级线四：下载量', 'account'),
('psdlfive', '0', '降级线五：下载量', 'account'),
('psratiotwo', '0.3', '降级线二：分享率', 'account'),
('psratiothree', '0.2', '降级线三：分享率', 'account'),
('psratiofour', '0.1', '降级线四：分享率', 'account'),
('psratiofive', '0.05', '降级线五：分享率', 'account'),
-- 种子设定
('randomtwouphalfdown', '0.01', '随机 2x50% 概率', 'torrent'),
('halfleechbecome', '86400', '50% 促销默认时长（秒）', 'torrent'),
('twoupfreebecome', '86400', '2x免费 促销默认时长（秒）', 'torrent'),
('twouphalfleechbecome', '86400', '2x50% 促销默认时长（秒）', 'torrent'),
('thirtypercentleechbecome', '86400', '30% 促销默认时长（秒）', 'torrent'),
('expiretwoup', 'yes', '2x 到期回落普通', 'torrent'),
('expirehalfleech', 'yes', '50% 到期回落普通', 'torrent'),
('expiretwoupfree', 'yes', '2x免费 到期回落普通', 'torrent'),
('expiretwouphalfleech', 'yes', '2x50% 到期回落普通', 'torrent'),
('expirethirtypercent', 'yes', '30% 到期回落普通', 'torrent'),
('max_price', '100000', '种子最高售价（火花）', 'torrent'),
('tax_factor', '0', '税率系数', 'torrent'),
('paid_torrent_enabled', 'no', '开启付费种子', 'torrent'),
('sticky_first_level_background_color', '#FFD700', '一级置顶背景色', 'torrent'),
('sticky_second_level_background_color', '#FF8FC7', '二级置顶背景色', 'torrent'),
('claim_price', '1000', '认领保种消耗（火花）', 'torrent'),
('claim_timeout', '604800', '认领窗口（秒）', 'torrent'),
('claim_min_size', '0', '认领最小体积（字节）', 'torrent'),
('claim_min_seeders', '0', '认领做种数下限', 'torrent'),
('claim_max_per_user', '100', '单人认领上限（个）', 'torrent'),
-- 附件设定
('savedirectorytype', 'local', '附件存储方式', 'attachment'),
('watermark', 'no', '开启图片水印', 'attachment'),
('add_watermark_to_thumbnail', 'no', '缩略图叠加水印', 'attachment'),
('image_thumbnails', 'yes', '生成图片缩略图', 'attachment'),
('alternative_thumbnail_size', 'no', '启用备用缩略图尺寸', 'attachment'),
('image_size_for_watermark', '500', '水印适用最小边长（像素）', 'attachment'),
('attachment_authority', '0', '附件查看最低等级', 'attachment'),
('http_directory', '', '附件 HTTP 访问目录', 'attachment'),
-- 其他设定（版本信息卡，只读）
('mainversion', '1.0.0', '主版本号', 'misc'),
('subversion', 'build', '次版本号', 'misc'),
('releasedate', '2026-09-11', '发布日期', 'misc'),
('website', 'https://github.com/fluxtorrent', '项目主页', 'misc')
ON CONFLICT (name) DO NOTHING;

-- 既有键的分区归属补正（避免落进 misc）
UPDATE site_settings SET grp = 'basic' WHERE name = 'site_type';
UPDATE site_settings SET grp = 'main'  WHERE name IN ('announcement', 'module_showcase', 'module_textbooks');
UPDATE site_settings SET grp = 'misc'  WHERE name = 'settings_group_order';
UPDATE site_settings SET grp = 'main'  WHERE grp IS NULL AND name LIKE 'enable\_%';
UPDATE site_settings SET grp = 'main'  WHERE grp IS NULL AND name LIKE '%per\_page%';
UPDATE site_settings SET grp = 'misc'  WHERE name = 'cache_driver';
UPDATE site_settings SET grp = 'main'  WHERE name = 'invite_quota_user';
UPDATE site_settings SET grp = 'account' WHERE name IN ('peasant_warn_days', 'user_delete_days');

-- ============================================================================
-- 3) 播种 settings_meta —— 分块 INSERT（每块列数严格一致）
-- ============================================================================

-- 3.1 无附加属性的类型：text(纯文本) / yesno / classlevel / textarea / color
INSERT INTO settings_meta (name, type, label_zh, label_en, group_key, card_order) VALUES
-- 基础设定
('SITENAME', 'text', '站点名称', 'Site name', '基础信息', 1),
('site_name', 'text', '站点简称', 'Short name', '基础信息', 2),
('site_title', 'text', '站点副标题', 'Site title', '基础信息', 3),
('site_subtitle', 'text', '站点口号', 'Site subtitle', '基础信息', 4),
('SLOGAN', 'text', '站点标语', 'Slogan', '基础信息', 5),
('timezone', 'text', '站点时区', 'Timezone', '基础信息', 6),
('datefounded', 'text', '建站日期', 'Founded', '基础信息', 7),
('logo', 'text', '站点 Logo 地址', 'Logo', '外观', 1),
('carousel_images', 'textarea', '首页轮播图', 'Carousel images', '外观', 2),
('stylesheet_default', 'text', '默认样式', 'Default stylesheet', '外观', 3),
('nfo_view_style_default', 'text', 'NFO 查看样式', 'NFO view style', '外观', 4),
('site_language_enabled', 'yesno', '开启多语言', 'i18n enabled', '基础信息', 8),
('module_textbooks', 'yesno', '启用课本中心', 'Textbooks module', '模块开关', 1),
('module_showcase', 'yesno', '启用展示区', 'Showcase module', '模块开关', 2),
('enable_gzip', 'yesno', '启用 Gzip 压缩', 'Enable gzip', '模块开关', 3),
('enable_searchbox', 'yesno', '启用搜索框', 'Search box', '模块开关', 4),
('enable_shoutbox', 'yesno', '启用聊天室', 'Shoutbox', '模块开关', 5),
('enable_wiki', 'yesno', '启用 Wiki', 'Wiki', '模块开关', 6),
('enable_lastfm', 'yesno', '启用 Last.fm', 'Last.fm', '模块开关', 7),
('enable_uploadtop', 'yesno', '启用上传榜', 'Upload top', '模块开关', 8),
('enable_adult', 'yesno', '允许成人内容', 'Adult content', '模块开关', 9),
('enable_bank', 'yesno', '启用银行', 'Bank module', '模块开关', 10),
('enable_dressup', 'yesno', '启用装扮中心', 'Dressup module', '模块开关', 11),
('enable_farm', 'yesno', '启用农场', 'Farm module', '模块开关', 12),
('enable_forum', 'yesno', '启用论坛', 'Forum module', '模块开关', 13),
('enable_freeleech', 'yesno', '启用全站免费', 'Freeleech module', '模块开关', 14),
('enable_games', 'yesno', '启用娱乐屋', 'Games module', '模块开关', 15),
('enable_medal', 'yesno', '启用勋章', 'Medal module', '模块开关', 16),
('enable_offers', 'yesno', '启用候选区', 'Offers module', '模块开关', 17),
('enable_subtitle', 'yesno', '启用字幕区', 'Subtitle module', '模块开关', 18),
('enable_task', 'yesno', '启用任务中心', 'Task module', '模块开关', 19),
-- 主要设定
('register_open', 'yesno', '开放注册', 'Registration open', '注册与邀请', 7),
('enable_invite', 'yesno', '启用邀请系统', 'Invite system', '注册与邀请', 10),
('getInvitesByPromotion', 'yesno', '升级赠邀请', 'Invites on promotion', '注册与邀请', 11),
('SITEEMAIL', 'text', '站点邮箱', 'Site email', '站务信息', 1),
('reportemail', 'text', '举报处理邮箱', 'Report email', '站务信息', 2),
('PAYPALACCOUNT', 'text', '捐赠收款账户', 'PayPal account', '站务信息', 3),
('donation_custom', 'textarea', '捐赠说明', 'Donation note', '站务信息', 4),
('icplicense', 'text', 'ICP 备案号', 'ICP license', '站务信息', 5),
('torrent_dir', 'text', '种子文件目录', 'Torrent dir', '站务信息', 8),
('torrentnameprefix', 'text', '种子文件名前缀', 'Torrent name prefix', '站务信息', 9),
('bitbucket', 'text', '官方小组标识', 'Bitbucket', '站务信息', 10),
('announcement', 'textarea', '站点公告', 'Announcement', '站务信息', 17),
-- SMTP
('smtp_host', 'text', 'SMTP 服务器', 'SMTP host', 'SMTP 服务器', 1),
('smtp_from', 'text', '发件人地址', 'From address', 'SMTP 服务器', 4),
('smtpaddress', 'text', '发件显示地址', 'Display address', 'SMTP 服务器', 5),
('accountname', 'text', 'SMTP 账号', 'SMTP account', 'SMTP 账号', 1),
-- 安全设定
('securelogin', 'yesno', '强制安全登录', 'Secure login', '登录安全', 1),
('securetracker', 'yesno', 'Tracker HTTPS', 'Secure tracker', '登录安全', 2),
('enable_two_step', 'yesno', '启用两步验证', 'Two-step auth', '安全策略', 3),
('enable_passkey_login', 'yesno', '启用 Passkey 登录', 'Passkey login', '安全策略', 4),
('cheaterdet', 'yesno', '作弊者探测', 'Cheater detection', '安全策略', 5),
('enable_attendance_captcha', 'yesno', '签到验证码', 'Attendance captcha', '安全策略', 7),
('enable_image_verification', 'yesno', '图形验证码', 'Image verification', '安全策略', 8),
('use_challenge_response_authentication', 'yesno', '质询响应认证', 'Challenge response', '安全策略', 9),
('loginattemptwhitelist', 'textarea', '登录白名单 IP', 'Login whitelist', '登录安全', 6),
-- 权限设定（classlevel）
('defaultclass', 'classlevel', '新用户默认等级', 'Default class', '权限矩阵', 1),
('staffmem', 'classlevel', '站务最低等级', 'Staff minimum', '权限矩阵', 2),
('authority_visit', 'classlevel', '访问站点', 'Visit site', '权限矩阵', 10),
('authority_download', 'classlevel', '下载种子', 'Download', '权限矩阵', 11),
('authority_upload', 'classlevel', '上传种子', 'Upload', '权限矩阵', 12),
('authority_addcomment', 'classlevel', '发表评论', 'Add comment', '权限矩阵', 13),
('authority_uploadsubtitle', 'classlevel', '上传字幕', 'Upload subtitle', '权限矩阵', 14),
('authority_addoffer', 'classlevel', '提交候选', 'Add offer', '权限矩阵', 15),
('authority_askreseed', 'classlevel', '求续种', 'Ask reseed', '权限矩阵', 16),
('authority_beanonymous', 'classlevel', '匿名发布', 'Be anonymous', '权限矩阵', 17),
('authority_sendinvite', 'classlevel', '发送邀请', 'Send invite', '权限矩阵', 18),
('authority_buyinvite', 'classlevel', '购买邀请', 'Buy invite', '权限矩阵', 19),
('authority_viewinvite', 'classlevel', '查看邀请记录', 'View invites', '权限矩阵', 20),
('authority_applylink', 'classlevel', '申请友链', 'Apply link', '权限矩阵', 21),
('authority_seeuser', 'classlevel', '查看用户', 'See user', '权限矩阵', 22),
('authority_viewuserlist', 'classlevel', '查看用户列表', 'View user list', '权限矩阵', 23),
('authority_viewuseremail', 'classlevel', '查看用户邮箱', 'View user email', '权限矩阵', 24),
('authority_viewuserip', 'classlevel', '查看用户 IP', 'View user IP', '权限矩阵', 25),
('authority_viewuserhistory', 'classlevel', '查看用户历史', 'View user history', '权限矩阵', 26),
('authority_viewtorrenthistory', 'classlevel', '查看种子历史', 'View torrent history', '权限矩阵', 27),
('authority_viewstats', 'classlevel', '查看站点统计', 'View stats', '权限矩阵', 28),
('authority_viewpeers', 'classlevel', '查看 Peer 列表', 'View peers', '权限矩阵', 29),
('authority_viewanonymous', 'classlevel', '查看匿名发布者', 'View anonymous', '权限矩阵', 30),
('authority_topten', 'classlevel', '查看排行榜', 'Top ten', '权限矩阵', 31),
('authority_viewnfo', 'classlevel', '查看 NFO', 'View NFO', '权限矩阵', 32),
('authority_viewfunbox', 'classlevel', '查看趣味盒', 'View funbox', '权限矩阵', 33),
('authority_viewpoll', 'classlevel', '查看投票', 'View poll', '权限矩阵', 34),
('authority_viewofferlist', 'classlevel', '查看候选列表', 'View offer list', '权限矩阵', 35),
('authority_offervote', 'classlevel', '候选投票', 'Offer vote', '权限矩阵', 36),
('authority_pollvote', 'classlevel', '参与投票', 'Poll vote', '权限矩阵', 37),
('authority_funboxvote', 'classlevel', '趣味盒投票', 'Funbox vote', '权限矩阵', 38),
('authority_chat', 'classlevel', '聊天室发言', 'Chat', '权限矩阵', 39),
('authority_seelog', 'classlevel', '查看日志', 'See log', '权限矩阵', 40),
('authority_seebanned', 'classlevel', '查看封禁用户', 'See banned', '权限矩阵', 41),
('authority_seecheater', 'classlevel', '查看作弊者', 'See cheaters', '权限矩阵', 42),
('authority_seetorrentstructure', 'classlevel', '查看种子结构', 'See torrent structure', '权限矩阵', 43),
('authority_commanage', 'classlevel', '评论管理', 'Comment manage', '权限矩阵', 44),
('authority_postmanage', 'classlevel', '帖子管理', 'Post manage', '权限矩阵', 45),
('authority_pollmanage', 'classlevel', '投票管理', 'Poll manage', '权限矩阵', 46),
('authority_funmanage', 'classlevel', '趣味盒管理', 'Funbox manage', '权限矩阵', 47),
('authority_offermanage', 'classlevel', '候选管理', 'Offer manage', '权限矩阵', 48),
('authority_newsmanage', 'classlevel', '公告管理', 'News manage', '权限矩阵', 49),
('authority_movetorrent', 'classlevel', '移动种子分类', 'Move torrent', '权限矩阵', 50),
('authority_torrentsticky', 'classlevel', '置顶种子', 'Torrent sticky', '权限矩阵', 51),
('authority_torrentmanage', 'classlevel', '种子管理', 'Torrent manage', '权限矩阵', 52),
('authority_forummanage', 'classlevel', '版块管理', 'Forum manage', '权限矩阵', 53),
('authority_linkmanage', 'classlevel', '友链管理', 'Link manage', '权限矩阵', 54),
('authority_staffmem', 'classlevel', '群发管理组信箱', 'Staff mail', '权限矩阵', 55),
('authority_editstaffbox', 'classlevel', '编辑管理组信箱', 'Edit staffbox', '权限矩阵', 56),
-- 次要设定
('titlekeywords', 'text', '标题关键词', 'Title keywords', 'SEO 与统计', 1),
('metakeywords', 'text', 'META 关键词', 'Meta keywords', 'SEO 与统计', 2),
('metadescription', 'textarea', 'META 描述', 'Meta description', 'SEO 与统计', 3),
('cssdate', 'text', 'CSS 版本日期', 'CSS date', 'SEO 与统计', 5),
-- 火花设定
('enable_bonus', 'yesno', '启用火花系统', 'Bonus enabled', '签到奖励', 1),
('enable_attendance', 'yesno', '启用签到', 'Attendance enabled', '签到奖励', 2),
('vipstatus', 'yesno', 'VIP 身份标识', 'VIP status', '行为奖励', 16),
('freeleech_until', 'text', '全站免费截止', 'Freeleech until', '比率限制', 3),
-- 账号设定
('destroy_disabled', 'yesno', '物理删除被禁账号', 'Destroy disabled', '自动删号', 6),
('deletepeasant', 'yesno', '删除低分享率用户', 'Delete peasant', '等级升降', 2),
('enable_hr', 'yesno', '启用 H&R', 'Enable HR', '等级升降', 3),
-- 种子设定
('expirefree', 'yesno', '免费到期回落', 'Expire free', '促销时长', 8),
('expirehalfleech', 'yesno', '50% 到期回落', 'Expire half leech', '促销时长', 9),
('expiretwoup', 'yesno', '2x 到期回落', 'Expire 2x', '促销时长', 10),
('expiretwoupfree', 'yesno', '2x免费 到期回落', 'Expire 2x free', '促销时长', 11),
('expiretwouphalfleech', 'yesno', '2x50% 到期回落', 'Expire 2x half', '促销时长', 12),
('expirethirtypercent', 'yesno', '30% 到期回落', 'Expire 30%', '促销时长', 13),
('uploaderdouble', 'yesno', '发布者下载双倍', 'Uploader double', '大种与热门', 5),
('deldeadtorrent', 'yesno', '自动删无种', 'Delete dead torrent', '大种与热门', 6),
('enable_torrent_upload', 'yesno', '允许上传种子', 'Torrent upload', '大种与热门', 7),
('torrent_autoswitch', 'yesno', '自动切换促销', 'Auto promotion switch', '大种与热门', 8),
('claim_enabled', 'yesno', '开启保种认领', 'Claim enabled', '保种认领', 1),
('paid_torrent_enabled', 'yesno', '开启付费种子', 'Paid torrent', '置顶与付费', 3),
('sticky_first_level_background_color', 'color', '一级置顶背景色', 'Sticky L1 color', '置顶与付费', 1),
('sticky_second_level_background_color', 'color', '二级置顶背景色', 'Sticky L2 color', '置顶与付费', 2),
-- 附件设定
('enableattach', 'yesno', '开启附件', 'Enable attachments', '附件总开关', 1),
('savedirectory', 'text', '附件保存目录', 'Save directory', '附件总开关', 2),
('http_directory', 'text', '附件 HTTP 目录', 'HTTP directory', '附件总开关', 4),
('attachment_authority', 'classlevel', '附件查看最低等级', 'Attachment authority', '权限', 1),
('image_thumbnails', 'yesno', '生成缩略图', 'Image thumbnails', '缩略图', 1),
('alternative_thumbnail_size', 'yesno', '启用备用尺寸', 'Alt thumbnail size', '缩略图', 6),
('watermark', 'yesno', '开启水印', 'Watermark', '水印', 1),
('add_watermark_to_thumbnail', 'yesno', '缩略图加水印', 'Watermark thumbnail', '水印', 5),
-- 广告设定
('enablead', 'yesno', '开启广告', 'Enable ads', '广告', 1),
('enable_ad', 'yesno', '开启广告（兼容键）', 'Enable ads (alt)', '广告', 2),
('enablenoad', 'yesno', '允许用户免广告', 'Allow no-ad', '广告', 3),
('enablebonusnoad', 'yesno', '允许火花免广告', 'Bonus no-ad', '广告', 4)
ON CONFLICT (name) DO NOTHING;

-- 3.2 数字字段（unit / min / max / step）
INSERT INTO settings_meta (name, type, label_zh, label_en, unit, min, max, step, group_key, card_order) VALUES
('iniupload', 'number', '初始上传量', 'Initial upload', '字节', 0, NULL, 1, '注册与邀请', 1),
('invite_count', 'number', '默认邀请名额', 'Default invites', '个', 0, 1000, 1, '注册与邀请', 2),
('tmp_invite_count', 'number', '临时邀请名额', 'Temp invites', '个', 0, 1000, 1, '注册与邀请', 3),
('invite_timeout', 'number', '邀请码有效期', 'Invite TTL', '秒', 0, NULL, 1, '注册与邀请', 4),
('invite_quota_user', 'number', '周邀请配额', 'Weekly invite quota', '个', 0, 1000, 1, '注册与邀请', 5),
('signup_timeout', 'number', '注册超时', 'Signup timeout', '秒', 0, NULL, 1, '注册与邀请', 6),
('upload_deny_approval_deny_count', 'number', '上传禁令阈值', 'Upload deny threshold', '次', 0, 999, 1, '注册与邀请', 9),
('maxusers', 'number', '用户数上限', 'Max users', '个', 0, NULL, 1, '注册与邀请', 12),
('postsperpage', 'number', '每页帖子数', 'Posts per page', '条', 1, 500, 1, '页面分页', 1),
('topicsperpage', 'number', '每页主题数', 'Topics per page', '条', 1, 500, 1, '页面分页', 2),
('torrentsperpage', 'number', '每页种子数', 'Torrents per page', '条', 1, 500, 1, '页面分页', 3),
('maxnewsnum', 'number', '首页公告条数', 'News per page', '条', 0, 50, 1, '页面分页', 4),
('max_pm_per_page', 'number', '每页短讯数', 'PM per page', '条', 1, 200, 1, '页面分页', 5),
('posts_per_page_default', 'number', '帖列表默认每页', 'Default posts', '条', 1, 500, 1, '页面分页', 6),
('topics_per_page_default', 'number', '主题列表默认每页', 'Default topics', '条', 1, 500, 1, '页面分页', 7),
('torrents_per_page_default', 'number', '种子列表默认每页', 'Default torrents', '条', 1, 500, 1, '页面分页', 8),
('pm_per_page_default', 'number', '短讯默认每页', 'Default PM', '条', 1, 200, 1, '页面分页', 9),
('max_torrent_size', 'number', '种子体积上限', 'Max torrent size', '字节', 0, NULL, 1, '上传限制', 1),
('maxsubsize', 'number', '字幕体积上限', 'Max subtitle size', '字节', 0, NULL, 1, '上传限制', 2),
('max_dead_torrent_time', 'number', '死种判定时长', 'Dead torrent time', '秒', 0, NULL, 1, '上传限制', 3),
('minoffervotes', 'number', '候选最少票数', 'Min offer votes', '票', 0, 1000, 1, '候选规则', 1),
('offervotetimeout', 'number', '候选投票超时', 'Offer vote timeout', '秒', 0, NULL, 1, '候选规则', 2),
('offeruptimeout', 'number', '候选上架超时', 'Offer up timeout', '秒', 0, NULL, 1, '候选规则', 3),
('offer_skip_approved_count', 'number', '候选转正所需赞数', 'Offer skip count', '票', 0, 1000, 1, '候选规则', 4),
('announce_interval', 'number', '客户端汇报间隔', 'Announce interval', '秒', 60, 86400, 1, '系统调度', 1),
('autoclean_interval_one', 'number', '自动清理间隔一', 'Autoclean interval 1', '秒', 0, NULL, 1, '系统调度', 2),
('annintertwo', 'number', '公告二显示时长', 'Announce 2 duration', '秒', 0, NULL, 1, '站务信息', 13),
('anninterthree', 'number', '公告三显示时长', 'Announce 3 duration', '秒', 0, NULL, 1, '站务信息', 14),
('annintertwoage', 'number', '公告二提前量', 'Announce 2 lead', '天', 0, 3650, 1, '站务信息', 15),
('anninterthreeage', 'number', '公告三提前量', 'Announce 3 lead', '天', 0, 3650, 1, '站务信息', 16),
('smtp_port', 'number', 'SMTP 端口', 'SMTP port', '端口', 1, 65535, 1, 'SMTP 服务器', 2),
('smtpport', 'number', 'SMTP 备用端口', 'SMTP port (alt)', '端口', 1, 65535, 1, 'SMTP 服务器', 3),
('maxip', 'number', '同 IP 账号数上限', 'Max IP', '个', 1, 999, 1, '登录安全', 4),
('maxloginattempts', 'number', '失败登录锁定阈值', 'Max login attempts', '次', 1, 99, 1, '登录安全', 5),
('login_secret_lifetime', 'number', '登录密钥有效期', 'Login secret TTL', '秒', 60, NULL, 1, '登录安全', 7),
('perseeding', 'number', '每个做种基础火花', 'Per seeding', '火花/种', 0, NULL, 0.01, '做种公式', 1),
('maxseeding', 'number', '计费做种上限', 'Max seeding', '个', 0, 10000, 1, '做种公式', 2),
('tzero', 'number', '公式 T0', 'Formula T0', '小时', 0, 1000, 0.1, '做种公式', 3),
('nzero', 'number', '公式 N0', 'Formula N0', '人数', 0, 10000, 1, '做种公式', 4),
('bzero', 'number', '公式 B0', 'Formula B0', '火花', 0, NULL, 1, '做种公式', 5),
('l', 'number', '公式 L', 'Formula L', '系数', 0, NULL, 1, '做种公式', 6),
('zero_bonus_factor', 'number', '零火花种子系数', 'Zero bonus factor', '0-1', 0, 1, 0.01, '做种公式', 7),
('donortimes', 'number', '捐赠者火花倍数', 'Donor multiplier', '倍', 1, 100, 0.1, '做种公式', 8),
('bonus_per_seed', 'number', '做种基础火花（旧键）', 'Bonus per seed', '火花', 0, NULL, 0.01, '做种公式', 9),
('uploadtorrent', 'number', '发布种子奖励', 'Upload torrent reward', '火花', 0, NULL, 0.5, '行为奖励', 1),
('uploadsubtitle', 'number', '发布字幕奖励', 'Upload subtitle reward', '火花', 0, NULL, 0.5, '行为奖励', 2),
('starttopic', 'number', '发主题奖励', 'Start topic reward', '火花', 0, NULL, 0.5, '行为奖励', 3),
('makepost', 'number', '回帖奖励', 'Make post reward', '火花', 0, NULL, 0.5, '行为奖励', 4),
('addcomment', 'number', '评论奖励', 'Comment reward', '火花', 0, NULL, 0.5, '行为奖励', 5),
('pollvote', 'number', '投票奖励', 'Poll vote reward', '火花', 0, NULL, 0.5, '行为奖励', 6),
('offervote', 'number', '候选投票奖励', 'Offer vote reward', '火花', 0, NULL, 0.5, '行为奖励', 7),
('funboxvote', 'number', '趣味盒投票奖励', 'Funbox vote reward', '火花', 0, NULL, 0.5, '行为奖励', 8),
('saythanks', 'number', '说谢谢奖励', 'Say thanks reward', '火花', 0, NULL, 0.5, '行为奖励', 9),
('receivethanks', 'number', '被感谢奖励', 'Receive thanks reward', '火花', 0, NULL, 0.5, '行为奖励', 10),
('funboxreward', 'number', '趣味盒作品奖励', 'Funbox reward', '火花', 0, NULL, 0.5, '行为奖励', 11),
('prolinkpoint', 'number', '推广链接奖励', 'Prolink reward', '火花', 0, NULL, 0.5, '行为奖励', 12),
('prolinktime', 'number', '推广链接有效期', 'Prolink TTL', '秒', 0, NULL, 1, '行为奖励', 13),
('official_addition', 'number', '官方发布加成', 'Official addition', '%', 0, 1000, 1, '行为奖励', 14),
('harem_addition', 'number', '后宫加成', 'Harem addition', '%', 0, 1000, 1, '行为奖励', 15),
('onegbupload', 'number', '1GB 上传量价格', 'Price 1GB upload', '火花', 0, NULL, 1, '商店价格', 1),
('fivegbupload', 'number', '5GB 上传量价格', 'Price 5GB upload', '火花', 0, NULL, 1, '商店价格', 2),
('tengbupload', 'number', '10GB 上传量价格', 'Price 10GB upload', '火花', 0, NULL, 1, '商店价格', 3),
('hundredgbupload', 'number', '100GB 上传量价格', 'Price 100GB upload', '火花', 0, NULL, 1, '商店价格', 4),
('tengbdownload', 'number', '10GB 下载量价格', 'Price 10GB download', '火花', 0, NULL, 1, '商店价格', 5),
('hundredgbdownload', 'number', '100GB 下载量价格', 'Price 100GB download', '火花', 0, NULL, 1, '商店价格', 6),
('oneinvite', 'number', '邀请价格', 'Price invite', '火花', 0, NULL, 1, '商店价格', 7),
('one_tmp_invite', 'number', '临时邀请价格', 'Price temp invite', '火花', 0, NULL, 1, '商店价格', 8),
('customtitle', 'number', '自定义头衔价格', 'Price custom title', '火花', 0, NULL, 1, '商店价格', 9),
('change_username_card', 'number', '改名卡价格', 'Price rename card', '火花', 0, NULL, 1, '商店价格', 10),
('rainbow_id', 'number', '彩虹 ID 价格', 'Price rainbow ID', '火花', 0, NULL, 1, '商店价格', 11),
('cancel_hr', 'number', '取消 H&R 价格', 'Price cancel HR', '火花', 0, NULL, 1, '商店价格', 12),
('attendance_card', 'number', '补签卡价格', 'Price makeup card', '火花', 0, NULL, 1, '商店价格', 13),
('basictax', 'number', '交易基础税', 'Basic tax', '火花', 0, NULL, 1, '交易税', 1),
('taxpercentage', 'number', '交易税率', 'Tax percentage', '%', 0, 100, 0.1, '交易税', 2),
('noad_hours', 'number', '免广告时长', 'No-ad hours', '小时', 0, NULL, 1, '交易税', 3),
('ratiolimit', 'number', '低分享率限制', 'Ratio limit', '比率', 0, 100, 0.01, '比率限制', 1),
('dlamountlimit', 'number', '低下载量限制', 'Download limit', '字节', 0, NULL, 1, '比率限制', 2),
('attendance_bonus', 'number', '签到基础奖励（旧键）', 'Attendance bonus', '火花', 0, NULL, 1, '签到奖励', 3),
('attendance_initial', 'number', '签到基础奖励', 'Attendance initial', '火花', 0, NULL, 1, '签到奖励', 4),
('attendance_step', 'number', '连签每日递增', 'Attendance step', '火花', 0, NULL, 1, '签到奖励', 5),
('attendance_max', 'number', '签到单日封顶', 'Attendance max', '火花', 0, NULL, 1, '签到奖励', 6),
('deletepacked', 'number', '停用账号删号', 'Delete packed', '天', 0, 3650, 1, '自动删号', 1),
('deleteunpacked', 'number', '未登录账号删号', 'Delete unpacked', '天', 0, 3650, 1, '自动删号', 2),
('deletenotransfer', 'number', '无流量账号删号', 'Delete no transfer', '天', 0, 3650, 1, '自动删号', 3),
('deletenotransfertwo', 'number', '无流量账号二次删号', 'Delete no transfer 2', '天', 0, 3650, 1, '自动删号', 4),
('user_delete_days', 'number', '注册未验证删号', 'Delete inactive days', '天', 0, 3650, 1, '自动删号', 5),
('peasant_warn_days', 'number', '降级警告天数', 'Peasant warn days', '天', 0, 365, 1, '等级升降', 1),
('hr_hours', 'number', 'H&R 考核时长', 'HR hours', '小时', 0, 10000, 1, '等级升降', 4),
('psdlone', 'number', '降级线一：下载量', 'PSDL 1', '字节', 0, NULL, 1, '等级升降', 5),
('psratioone', 'number', '降级线一：分享率', 'PSRatio 1', '比率', 0, 100, 0.01, '等级升降', 6),
('psdltwo', 'number', '降级线二：下载量', 'PSDL 2', '字节', 0, NULL, 1, '等级升降', 7),
('psratiotwo', 'number', '降级线二：分享率', 'PSRatio 2', '比率', 0, 100, 0.01, '等级升降', 8),
('psdlthree', 'number', '降级线三：下载量', 'PSDL 3', '字节', 0, NULL, 1, '等级升降', 9),
('psratiothree', 'number', '降级线三：分享率', 'PSRatio 3', '比率', 0, 100, 0.01, '等级升降', 10),
('psdlfour', 'number', '降级线四：下载量', 'PSDL 4', '字节', 0, NULL, 1, '等级升降', 11),
('psratiofour', 'number', '降级线四：分享率', 'PSRatio 4', '比率', 0, 100, 0.01, '等级升降', 12),
('psdlfive', 'number', '降级线五：下载量', 'PSDL 5', '字节', 0, NULL, 1, '等级升降', 13),
('psratiofive', 'number', '降级线五：分享率', 'PSRatio 5', '比率', 0, 100, 0.01, '等级升降', 14),
('randomfree', 'number', '随机免费概率', 'Random free', '概率', 0, 1, 0.001, '促销概率', 1),
('randomtwoup', 'number', '随机 2x 概率', 'Random 2x', '概率', 0, 1, 0.001, '促销概率', 2),
('randomhalfleech', 'number', '随机 50% 概率', 'Random half leech', '概率', 0, 1, 0.001, '促销概率', 3),
('randomtwoupfree', 'number', '随机 2x免费 概率', 'Random 2x free', '概率', 0, 1, 0.001, '促销概率', 4),
('randomthirtypercentdown', 'number', '随机 30% 概率', 'Random 30% down', '概率', 0, 1, 0.001, '促销概率', 5),
('randomtwouphalfdown', 'number', '随机 2x50% 概率', 'Random 2x half down', '概率', 0, 1, 0.001, '促销概率', 6),
('largesize', 'number', '大种标准', 'Large size', '字节', 0, NULL, 1, '大种与热门', 1),
('hotdays', 'number', '热门天数', 'Hot days', '天', 0, 365, 1, '大种与热门', 3),
('hotseeder', 'number', '热门做种数阈值', 'Hot seeders', '个', 0, 100000, 1, '大种与热门', 4),
('freebecome', 'number', '免费默认时长', 'Free duration', '秒', 0, NULL, 1, '促销时长', 1),
('halfleechbecome', 'number', '50% 默认时长', 'Half leech duration', '秒', 0, NULL, 1, '促销时长', 2),
('twoupbecome', 'number', '2x 默认时长', '2x duration', '秒', 0, NULL, 1, '促销时长', 3),
('twoupfreebecome', 'number', '2x免费 默认时长', '2x free duration', '秒', 0, NULL, 1, '促销时长', 4),
('twouphalfleechbecome', 'number', '2x50% 默认时长', '2x half duration', '秒', 0, NULL, 1, '促销时长', 5),
('thirtypercentleechbecome', 'number', '30% 默认时长', '30% duration', '秒', 0, NULL, 1, '促销时长', 6),
('normalbecome', 'number', '促销回落普通时长', 'Normal duration', '秒', 0, NULL, 1, '促销时长', 7),
('claim_torrent_ttl', 'number', '保种有效期', 'Claim TTL', '秒', 0, NULL, 1, '保种认领', 2),
('claim_bonus_multiplier', 'number', '保种火花倍数', 'Claim bonus multiplier', '倍', 0, 100, 0.1, '保种认领', 3),
('claim_price', 'number', '认领消耗火花', 'Claim price', '火花', 0, NULL, 1, '保种认领', 4),
('claim_timeout', 'number', '认领窗口', 'Claim timeout', '秒', 0, NULL, 1, '保种认领', 5),
('claim_min_size', 'number', '认领最小体积', 'Claim min size', '字节', 0, NULL, 1, '保种认领', 6),
('claim_min_seeders', 'number', '认领做种数下限', 'Claim min seeders', '个', 0, 100000, 1, '保种认领', 7),
('claim_max_per_user', 'number', '单人认领上限', 'Claim max per user', '个', 0, 10000, 1, '保种认领', 8),
('max_price', 'number', '种子最高售价', 'Max price', '火花', 0, NULL, 1, '置顶与付费', 4),
('tax_factor', 'number', '税率系数', 'Tax factor', '系数', 0, 100, 0.01, '置顶与付费', 5),
('thumbquality', 'number', '缩略图质量', 'Thumb quality', '%', 1, 100, 1, '缩略图', 3),
('thumbwidth', 'number', '缩略图宽', 'Thumb width', '像素', 1, 10000, 1, '缩略图', 4),
('thumbheight', 'number', '缩略图高', 'Thumb height', '像素', 1, 10000, 1, '缩略图', 5),
('watermarkquality', 'number', '水印 JPEG 质量', 'Watermark quality', '%', 1, 100, 1, '水印', 3),
('image_size_for_watermark', 'number', '水印最小边长', 'Watermark min size', '像素', 0, 100000, 1, '水印', 4),
('bonusnoadpoint', 'number', '免广告价格', 'No-ad price', '火花', 0, NULL, 1, '广告', 5),
('bonusnoadtime', 'number', '免广告时长', 'No-ad days', '天', 0, 3650, 1, '广告', 6),
('adclickbonus', 'number', '点击广告奖励', 'Ad click bonus', '火花', 0, NULL, 0.5, '广告', 7)
ON CONFLICT (name) DO NOTHING;

-- 3.3 枚举字段
INSERT INTO settings_meta (name, type, label_zh, label_en, options, group_key, card_order) VALUES
('site_type', 'enum', '站点类型', 'Site type',
 '[{"v":"education","l":"教育站"},{"v":"movie","l":"影视站"},{"v":"music","l":"音乐站"},{"v":"anime","l":"动漫站"},{"v":"ebook","l":"电子书站"},{"v":"general","l":"综合站"},{"v":"sports","l":"体育站"},{"v":"game","l":"游戏站"},{"v":"software","l":"软件站"},{"v":"documentary","l":"纪录片站"},{"v":"lossless","l":"无损音乐站"}]'::jsonb,
 '基础信息', 9),
('default_language', 'enum', '默认语言', 'Default language',
 '[{"v":"zh-CN","l":"简体中文"},{"v":"zh-TW","l":"繁體中文"},{"v":"en","l":"English"}]'::jsonb, '基础信息', 10),
('defaultlang', 'enum', '默认语言（NP 口径）', 'Default lang',
 '[{"v":"zh-CN","l":"简体中文"},{"v":"zh-TW","l":"繁體中文"},{"v":"en","l":"English"}]'::jsonb, '基础信息', 11),
('verification', 'enum', '注册验证方式', 'Verification',
 '[{"v":"none","l":"关闭"},{"v":"email","l":"邮箱验证"},{"v":"img","l":"图形验证码"},{"v":"emailimg","l":"邮箱+图形"}]'::jsonb, '注册与邀请', 8),
('imdb_language', 'enum', 'IMDb 信息语言', 'IMDb language',
 '[{"v":"zh-CN","l":"简体中文"},{"v":"en-US","l":"English"}]'::jsonb, '站务信息', 7),
('encryption', 'enum', '加密方式', 'Encryption',
 '[{"v":"ssl","l":"SSL"},{"v":"tls","l":"TLS"},{"v":"none","l":"无"}]'::jsonb, 'SMTP 服务器', 6),
('login_type', 'enum', '登录方式', 'Login type',
 '[{"v":"default","l":"用户名/邮箱 + 密码"},{"v":"passkey","l":"Passkey"},{"v":"two_step","l":"两步验证"}]'::jsonb, '安全策略', 1),
('guest_visit_type', 'enum', '游客访问处理', 'Guest visit type',
 '[{"v":"redirect","l":"跳转登录页"},{"v":"static_page","l":"静态说明页"}]'::jsonb, '安全策略', 2),
('cheater_detection_level', 'enum', '作弊探测等级', 'Cheater level',
 '[{"v":"off","l":"关闭"},{"v":"loose","l":"宽松"},{"v":"normal","l":"标准"},{"v":"strict","l":"严格"}]'::jsonb, '安全策略', 6),
('largepro', 'enum', '大种自动促销', 'Large promotion',
 '[{"v":"free","l":"免费"},{"v":"2up","l":"2x"},{"v":"halfleech","l":"50%"},{"v":"2upfree","l":"2x免费"},{"v":"none","l":"不促销"}]'::jsonb, '大种与热门', 2),
('savedirectorytype', 'enum', '附件存储方式', 'Storage type',
 '[{"v":"local","l":"本地磁盘"},{"v":"s3","l":"对象存储"}]'::jsonb, '附件总开关', 3),
('thumbnailtype', 'enum', '缩略图方式', 'Thumbnail type',
 '[{"v":"no","l":"不处理"},{"v":"createthumb","l":"生成缩略图"},{"v":"resizebigimg","l":"压缩大图"}]'::jsonb, '缩略图', 2),
('watermarkpos', 'enum', '水印位置', 'Watermark position',
 '[{"v":"no","l":"关闭"},{"v":"tl","l":"左上"},{"v":"tr","l":"右上"},{"v":"bl","l":"左下"},{"v":"br","l":"右下"},{"v":"center","l":"居中"}]'::jsonb, '水印', 2)
ON CONFLICT (name) DO NOTHING;

-- 3.4 密文字段（secret）
INSERT INTO settings_meta (name, type, label_zh, label_en, secret, group_key, card_order) VALUES
('accountpassword', 'password', 'SMTP 密码', 'SMTP password', true, 'SMTP 账号', 2),
('analyticscode', 'password', '统计代码', 'Analytics code', true, 'SEO 与统计', 4)
ON CONFLICT (name) DO NOTHING;

-- 3.5 只读字段（版本信息卡）
INSERT INTO settings_meta (name, type, label_zh, label_en, readonly, group_key, card_order) VALUES
('mainversion', 'text', '主版本号', 'Main version', true, '版本信息', 1),
('subversion', 'text', '次版本号', 'Sub version', true, '版本信息', 2),
('releasedate', 'text', '发布日期', 'Release date', true, '版本信息', 3),
('website', 'text', '项目主页', 'Website', true, '版本信息', 4)
ON CONFLICT (name) DO NOTHING;

-- 3.6 隐藏字段（基础设施键，不作为站点设定项渲染）
INSERT INTO settings_meta (name, type, label_zh, label_en, visible, group_key, card_order) VALUES
('settings_group_order', 'text', '分组顺序（元数据）', 'Group order (meta)', false, '其他', 99),
('cache_driver', 'text', '缓存驱动（基础设施）', 'Cache driver (infra)', false, '其他', 98)
ON CONFLICT (name) DO NOTHING;

-- 3.7 文本校验规则（URL / 必填 / ID 列表）
INSERT INTO settings_meta (name, type, label_zh, label_en, options, group_key, card_order) VALUES
('BASEURL', 'text', '站点根 URL', 'Base URL', '{"rule":"url","required":true}'::jsonb, '基础信息', 12),
('announce_url', 'text', 'Tracker 地址', 'Announce URL', '{"rule":"url","required":true}'::jsonb, '基础信息', 13),
('extforumurl', 'text', '外部论坛地址', 'External forum', '{"rule":"url"}'::jsonb, '站务信息', 6),
('https_announce_url', 'text', 'HTTPS announce 地址', 'HTTPS announce', '{"rule":"url"}'::jsonb, '登录安全', 3),
('specialcat', 'text', '特殊分类 ID', 'Special categories', '{"rule":"csv_ids"}'::jsonb, '站务信息', 11),
('browsecat', 'text', '浏览分类 ID', 'Browse categories', '{"rule":"csv_ids"}'::jsonb, '站务信息', 12),
('protected_forum', 'text', '受保护版块 ID', 'Protected forums', '{"rule":"csv_ids"}'::jsonb, '其他', 1),
('SITENAME', 'text', '站点名称', 'Site name', '{"required":true}'::jsonb, '基础信息', 1)
ON CONFLICT (name) DO UPDATE SET
  options = EXCLUDED.options,
  label_zh = EXCLUDED.label_zh,
  label_en = EXCLUDED.label_en,
  group_key = EXCLUDED.group_key,
  card_order = EXCLUDED.card_order;

-- 3.8 成对字段（签到连签：天数 ↔ 奖励）
INSERT INTO settings_meta (name, type, label_zh, label_en, unit, group_key, card_order) VALUES
('attendance_continuous_day', 'pair', '连签里程碑天数', 'Attendance milestone days', NULL, '签到奖励', 7),
('attendance_continuous_value', 'pair', '连签里程碑奖励', 'Attendance milestone rewards', '火花', '签到奖励', 8)
ON CONFLICT (name) DO NOTHING;

-- ============================================================================
-- 4) 关键字段帮助说明（hint）
-- ============================================================================
UPDATE settings_meta m SET hint = v.hint
FROM (VALUES
  ('SITENAME', '全站标题、邮件署名与 RSS 频道名'),
  ('BASEURL', '含协议与端口；变更后 RSS / 邮件内链接同步更新'),
  ('announce_url', '变更后已发布的旧种子文件需重新下载'),
  ('site_type', '决定分类树与模块默认开关，切换后建议复核分类'),
  ('SLOGAN', '站内信息页顶部标语'),
  ('logo', '留空使用主题默认 Logo'),
  ('carousel_images', '每行一个图片 URL；留空不展示轮播'),
  ('max_dead_torrent_time', '超过此时长无做种视为死种'),
  ('torrent_dir', '服务端绝对或相对路径，仅运维关注'),
  ('accountpassword', '留空表示保持不变；不回显明文'),
  ('analyticscode', '留空表示保持不变；不回显明文'),
  ('maxip', '同 IP 可同时登录的账号数上限'),
  ('maxloginattempts', '超过阈值后临时锁定该来源'),
  ('loginattemptwhitelist', '每行一个 IP 或 CIDR，白名单不受锁定限制'),
  ('destroy_disabled', '危险：将物理删除被禁账号，不可恢复'),
  ('deletepeasant', '危险：自动删除长期低分享率用户'),
  ('zero_bonus_factor', '取值 0-1，用于零做种/零下载场景的系数'),
  ('ratiolimit', '分享率低于此值时限制下载'),
  ('claim_price', '单次认领保种消耗的火花'),
  ('claim_torrent_ttl', '认领后需在此时长内完成保种'),
  ('paid_torrent_enabled', '开启后发布者可对种子定价，买家付费后下载'),
  ('max_price', '单一种子允许的最高售价'),
  ('tax_factor', '付费种子交易抽取的税率系数'),
  ('sticky_first_level_background_color', '列表置顶一级底色，须为合法 hex（如 #FFD700）'),
  ('sticky_second_level_background_color', '列表置顶二级底色，须为合法 hex'),
  ('watermark', '开启后上传图片自动叠加水印'),
  ('watermarkpos', '水印叠加位置'),
  ('attachment_authority', '低于此等级不可查看附件'),
  ('bonusnoadpoint', '用户购买免广告消耗的火花')
) AS v(name, hint) WHERE m.name = v.name;

-- 隐藏键的元数据不参与渲染，但保留分组信息以便审计追溯
UPDATE settings_meta SET visible = FALSE WHERE name IN ('settings_group_order', 'cache_driver');
