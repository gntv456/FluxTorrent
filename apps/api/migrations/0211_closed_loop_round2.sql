-- 0211：闭环复检修复批（2026-09-26 二轮）
--
-- H1：捐赠订单面板导航隐形——0209 种子给了 section='site'，
--     而前端 SECTION_ORDER 六值（dashboard/moderation/users/content/ops/system）
--     不含 'site'，条目被静默丢弃、侧边栏不可见。归入 'ops'（运维域）。
UPDATE staff_panel_entries
SET section = 'ops'
WHERE tab_key = 'donateorders' AND section NOT IN
    ('dashboard', 'moderation', 'users', 'content', 'ops', 'system');

-- M1：SMTP 凭据三键此前为死设置（0034 种下、零消费）——0211 起由
--     mailer::smtp_config 装配进 URL（见 mailer.rs），此处补 meta 描述对齐
UPDATE settings_meta
SET hint = 'SMTP 登录用户名（需要认证的发件服务填写；留空 = 匿名发信）'
WHERE name = 'accountname';

UPDATE settings_meta
SET hint = 'SMTP 登录密码（随用户名一并使用；保存后掩码显示）'
WHERE name = 'accountpassword';

UPDATE settings_meta
SET hint = '加密方式：ssl（465 端口）/ tls（587 STARTTLS）/ none（内网中继）'
WHERE name = 'encryption';
