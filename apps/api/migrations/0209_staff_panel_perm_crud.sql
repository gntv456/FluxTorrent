-- 0209：staff_panel_entries 权限键 + CRUD 基建（闭环审查 P1-7）
--
-- 现状：面板导航只按 min_class <= class_id 过滤，加减条目要写 SQL。
-- 本迁移：
--   1) 加 perm_key 列（可空 = 不按权限过滤，保持旧行为；非空 = 还需 user_can(perm_key)）；
--   2) 为既有 tab_key 回填合理权限键（条目与后端端点的 require_perm 对齐）；
--   3) CRUD 走 /admin/staffpanel-entries（无需迁移数据，仅列）。

ALTER TABLE staff_panel_entries
    ADD COLUMN IF NOT EXISTS perm_key text;

COMMENT ON COLUMN staff_panel_entries.perm_key IS
    '访问该面板条目所需权限键（authz perm 常量值）；NULL = 仅按 min_class 过滤';

-- 回填：与各面板后端端点的实际 require_perm 对齐（保守——只标能确定的）
UPDATE staff_panel_entries SET perm_key = 'settings.manage'
WHERE tab_key IN ('settings') AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'faq.manage'
WHERE tab_key IN ('faq') AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'rules.manage'
WHERE tab_key IN ('rules') AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'news.manage'
WHERE tab_key IN ('news') AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'user.status'
WHERE tab_key IN ('bans') AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'invite.view'
WHERE tab_key IN ('invites') AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'medal.manage'
WHERE tab_key IN ('medals') AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'prop.manage'
WHERE tab_key IN ('props') AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'user.amountbonus'
WHERE tab_key IN ('incrementbulk') AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'forums.manage'
WHERE tab_key IN ('forums') AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'torrent.manage'
WHERE tab_key IN ('torrents') AND perm_key IS NULL;

-- 新面板条目种子：捐赠订单 / 面板条目管理（0209 P1-6/P1-7 的运营入口）
INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key, perm_key)
SELECT 'admin', '捐赠订单', '/admin?tool=donateorders',
       '支付订单列表 / 对账合计 / 网关掉单手工补单',
       12, 'site', 93, 'donateorders', 'user.adjust'
WHERE NOT EXISTS (
    SELECT 1 FROM staff_panel_entries WHERE tab_key = 'donateorders'
);

INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key, perm_key)
SELECT 'sysop', '面板条目', '/admin?tool=panelentries',
       '后台导航条目增删改（分区/等级/权限键/模块键）',
       13, 'system', 99, 'panelentries', 'settings.manage'
WHERE NOT EXISTS (
    SELECT 1 FROM staff_panel_entries WHERE tab_key = 'panelentries'
);
