-- 0438: 死键摘除·第一批——authority/security/advertisement 三区（B3）。
--
-- 判据：fake_switch 基线（全仓零读取方，含 SQL 函数体面）∩ 当前可见键，
-- 已逐键 spot-check 无隐藏消费者（authority_* 族、bonusnoad 族均零命中）。
-- 与 0193/0434/0435 同口径：meta 行 + 值行同摘；fake_switch_baseline.json
-- 不动（墓碑机制）；pt_audit_mb_dead_setting_cards.py LEDGER 同批删号。
--
-- 分流定性（详见 _doc/设置页全面收口方案-2026-10-30.md）：
--   authority 47 键 = B 类：NP 按等级权限矩阵被自研 authz/perm_key 取代
--     （user_permissions 显式拒绝 + 菜单绑 perm_key + 99 守卫），留活键
--     staffmem（站务最低等级，有读取方）；
--   security 13 键 = A 类撤卡：2FA/Passkey 是用户级开关无站级总闸、登录
--     锁定真实阈值在 Redis acctlock 流程内（B5 接线恢复可调）、探测阈值
--     写死 SQL 是设计选择（MB5 判据）、guest_visit_type 对应功能从未实现；
--   advertisement 6 键 = C 类：广告系统未实现，记 backlog。

DELETE FROM settings_meta WHERE name IN (
    -- authority：NP 权限矩阵（47）
    'authority_addcomment', 'authority_addoffer', 'authority_applylink',
    'authority_askreseed', 'authority_beanonymous', 'authority_buyinvite',
    'authority_chat', 'authority_commanage', 'authority_download',
    'authority_editstaffbox', 'authority_forummanage',
    'authority_funboxvote', 'authority_funmanage', 'authority_linkmanage',
    'authority_movetorrent', 'authority_newsmanage',
    'authority_offermanage', 'authority_offervote', 'authority_pollmanage',
    'authority_pollvote', 'authority_postmanage', 'authority_seebanned',
    'authority_seecheater', 'authority_seelog',
    'authority_seetorrentstructure', 'authority_seeuser',
    'authority_sendinvite', 'authority_staffmem', 'authority_topten',
    'authority_torrentmanage', 'authority_torrentsticky',
    'authority_upload', 'authority_uploadsubtitle',
    'authority_viewanonymous', 'authority_viewfunbox',
    'authority_viewinvite', 'authority_viewnfo',
    'authority_viewofferlist', 'authority_viewpeers', 'authority_viewpoll',
    'authority_viewstats', 'authority_viewtorrenthistory',
    'authority_viewuseremail', 'authority_viewuserhistory',
    'authority_viewuserip', 'authority_viewuserlist', 'authority_visit',
    -- security：NP 时代站级开关（13）
    'cheater_detection_level', 'cheaterdet', 'enable_attendance_captcha',
    'enable_passkey_login', 'enable_two_step', 'guest_visit_type',
    'login_secret_lifetime', 'loginattemptwhitelist', 'maxip',
    'maxloginattempts', 'securelogin', 'securetracker',
    'use_challenge_response_authentication',
    -- advertisement：广告系统未实现（6）
    'adclickbonus', 'bonusnoadpoint', 'bonusnoadtime', 'enablead',
    'enablebonusnoad', 'enablenoad'
);
DELETE FROM site_settings WHERE name IN (
    'authority_addcomment', 'authority_addoffer', 'authority_applylink',
    'authority_askreseed', 'authority_beanonymous', 'authority_buyinvite',
    'authority_chat', 'authority_commanage', 'authority_download',
    'authority_editstaffbox', 'authority_forummanage',
    'authority_funboxvote', 'authority_funmanage', 'authority_linkmanage',
    'authority_movetorrent', 'authority_newsmanage',
    'authority_offermanage', 'authority_offervote', 'authority_pollmanage',
    'authority_pollvote', 'authority_postmanage', 'authority_seebanned',
    'authority_seecheater', 'authority_seelog',
    'authority_seetorrentstructure', 'authority_seeuser',
    'authority_sendinvite', 'authority_staffmem', 'authority_topten',
    'authority_torrentmanage', 'authority_torrentsticky',
    'authority_upload', 'authority_uploadsubtitle',
    'authority_viewanonymous', 'authority_viewfunbox',
    'authority_viewinvite', 'authority_viewnfo',
    'authority_viewofferlist', 'authority_viewpeers', 'authority_viewpoll',
    'authority_viewstats', 'authority_viewtorrenthistory',
    'authority_viewuseremail', 'authority_viewuserhistory',
    'authority_viewuserip', 'authority_viewuserlist', 'authority_visit',
    'cheater_detection_level', 'cheaterdet', 'enable_attendance_captcha',
    'enable_passkey_login', 'enable_two_step', 'guest_visit_type',
    'login_secret_lifetime', 'loginattemptwhitelist', 'maxip',
    'maxloginattempts', 'securelogin', 'securetracker',
    'use_challenge_response_authentication',
    'adclickbonus', 'bonusnoadpoint', 'bonusnoadtime', 'enablead',
    'enablebonusnoad', 'enablenoad'
);
