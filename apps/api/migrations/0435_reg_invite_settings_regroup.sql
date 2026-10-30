-- 0435: 注册/邀请设置归位——散在 6 个分区的键收拢 + 摘 9 个 NP 死键。
--
-- 背景（0434 姊妹批）：注册相关键是三代口径叠加，归位从未做过——同一件事
-- 的键散在 main(basic/main/注册与邀请 三张卡)、security、authority、account、
-- bonus(邀请)、注册与邀请 共 6 个分区里，站长配一次注册要在左导航跳 5 次。
--
-- 本批做两件事：
--
-- 1) 摘死键（与 0193/0434 同判据：全仓零读取方，含 SQL 函数体面）。
--    NP 对齐时代（0034/0039）登记的 9 键从未接过消费者；邀请面真实生效的
--    是 0204 配置化的 bonus 系四键（invites_weekly_class3 / bonus /
--    invite_ttl_hours / invite_admin_ttl_days，读方 invite_http.rs）：
--      iniupload 初始上传量 / invite_count 默认邀请名额 /
--      tmp_invite_count 临时邀请名额 / invite_timeout 邀请码有效期 /
--      invite_quota_user 周邀请配额 / enable_invite 启用邀请系统 /
--      getInvitesByPromotion 升级赠邀请 / signup_timeout 注册超时 /
--      defaultclass 新用户默认等级（真等级口径在 classes 表 id=1，非此键）
--
-- 2) 归位真键（只动 grp/group_key/card_order 展示坐标，不动值）：
--      registration_mode main/basic → 注册与邀请（头号开关放最前）
--      register_open   security → 注册与邀请（与 registration_mode 相邻，
--                       0431 的联动关系才看得见）
--      captcha_provider main/main → 注册与邀请（注册防机的主消费者；
--                       登录/申诉复用同驱动，放在人机组）
--      pow_difficulty_bits security → 注册与邀请（captcha_provider=pow 的
--                       配套难度）
--      bonus 系邀请四键 bonus/邀请 → 注册与邀请（单独成「邀请额度」卡，
--                       魔力分区回归纯经济）
--      upload_deny_approval_deny_count 是发种禁令（publish_http），与注册
--                       无关，误挂在「注册与邀请」卡——归位到 torrent 分区
--                       「上传限制」卡（0039 同名卡已有 6 键）。
--
-- card_order 重排为一条业务动线：门禁组(1-7) → 人机组(8-9) → 答题组(10-13)
-- → 名额组(14) → 邀请额度组(20-23)。fake_switch_baseline.json 不动（guard
-- 只认 INSERT 登记面，墓碑机制同 0193/0434）。

-- ===== 1) 摘 9 个 NP 死键 =====
DELETE FROM settings_meta WHERE name IN (
    'iniupload', 'invite_count', 'tmp_invite_count', 'invite_timeout',
    'invite_quota_user', 'enable_invite', 'getInvitesByPromotion',
    'signup_timeout', 'defaultclass'
);
DELETE FROM site_settings WHERE name IN (
    'iniupload', 'invite_count', 'tmp_invite_count', 'invite_timeout',
    'invite_quota_user', 'enable_invite', 'getInvitesByPromotion',
    'signup_timeout', 'defaultclass'
);

-- ===== 2) 真键归位 =====
-- 2.1 分区/卡片坐标
UPDATE site_settings SET grp = '注册与邀请'
 WHERE name IN ('registration_mode', 'register_open', 'captcha_provider',
                'pow_difficulty_bits', 'invites_weekly_class3',
                'invites_weekly_bonus', 'invite_ttl_hours',
                'invite_admin_ttl_days');

UPDATE settings_meta SET group_key = '注册与邀请'
 WHERE name IN ('registration_mode', 'register_open', 'captcha_provider',
                'pow_difficulty_bits');

UPDATE settings_meta SET group_key = '邀请额度'
 WHERE name IN ('invites_weekly_class3', 'invites_weekly_bonus',
                'invite_ttl_hours', 'invite_admin_ttl_days');

-- 发种禁令归位（挂错卡，非注册面）
UPDATE site_settings SET grp = 'torrent'
 WHERE name = 'upload_deny_approval_deny_count';
UPDATE settings_meta SET group_key = '上传限制'
 WHERE name = 'upload_deny_approval_deny_count';

-- 2.2 card_order 重排（同卡内业务动线）
UPDATE settings_meta SET card_order = 1  WHERE name = 'registration_mode';
UPDATE settings_meta SET card_order = 2  WHERE name = 'register_open';
UPDATE settings_meta SET card_order = 3  WHERE name = 'maxusers';
UPDATE settings_meta SET card_order = 4  WHERE name = 'reg_window';
UPDATE settings_meta SET card_order = 5  WHERE name = 'reg_ip_limit';
UPDATE settings_meta SET card_order = 6  WHERE name = 'reg_ip_window_hours';
UPDATE settings_meta SET card_order = 7  WHERE name = 'reg_ip_retention_days';
UPDATE settings_meta SET card_order = 8  WHERE name = 'reg_block_disposable';
UPDATE settings_meta SET card_order = 9  WHERE name = 'captcha_provider';
UPDATE settings_meta SET card_order = 10 WHERE name = 'pow_difficulty_bits';
UPDATE settings_meta SET card_order = 11 WHERE name = 'reg_quiz_enabled';
UPDATE settings_meta SET card_order = 12 WHERE name = 'reg_quiz_category';
UPDATE settings_meta SET card_order = 13 WHERE name = 'reg_quiz_ttl_minutes';
UPDATE settings_meta SET card_order = 14 WHERE name = 'reg_quiz_pass_required';
UPDATE settings_meta SET card_order = 20 WHERE name = 'invites_weekly_class3';
UPDATE settings_meta SET card_order = 21 WHERE name = 'invites_weekly_bonus';
UPDATE settings_meta SET card_order = 22 WHERE name = 'invite_ttl_hours';
UPDATE settings_meta SET card_order = 23 WHERE name = 'invite_admin_ttl_days';

-- hint 补一句引导：快捷开关与主枚举的联动关系（0431 已写过 register_open
-- 的 hint，这里给 registration_mode 也补上对偶说明）
UPDATE settings_meta
   SET hint = '新用户如何注册：邀请制 = 凭邀请码注册；开放注册 = 任何人可直接注册；开放注册 + 邮件激活 = 可直接注册，但须点验证邮件才能登录。本卡上方的「开放注册」是快捷开关，保存时会自动把本项在 开放注册/邀请制 间切换'
 WHERE name = 'registration_mode';
