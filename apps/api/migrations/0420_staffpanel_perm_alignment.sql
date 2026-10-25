-- 0420: 管理台导航项补权限键——点进去 403 的入口不再外露（2026-10-25 版主六轮实测）。
--
-- 现状：staff_panel_entries.perm_key 为 NULL 的条目只按 min_class 过滤，
-- 但面板首屏的读口/写口各带 require_perm，两者常不是同一档：
--   * 「作弊探测」min_class=90，/admin/cheaters 要 cheaters.view（95）
--   * 「举报处理」min_class=90，/admin/reports 要 reports.handle（92）
--   * 「金字字幕评选」min_class=90，授奖读口要 subtitles.grant（95）
--   * 「分类/标签/维度管理」min_class=93，读口要 categories.manage（99）
--   * 「导航菜单/运营配置/免费促销」min_class=93，读口要 settings.manage（99）
--   * 「绩效考核」min_class=93，payroll 要 jixiao.payroll.view（95）
--   * 「等级规则」min_class=93，/admin/roles-dict 要 roles.manage（99）
--   * 「重置密码」min_class=93，/admin/resetpass 要 user.resetpass（99）
--   * 「用户管理/用户日志」min_class=90，读口要 user.view（92）
-- 后果：受影响的等级看得到入口，点进去是 403「无权限」，写口同样 403。
--
-- 修法遵循 0209 的既有机制：perm_key 非空时 staffpanel 会逐条 user_can，
-- 与后端同一把键；NULL 条目继续按 min_class（工作台/概览等确无权限键）。
-- 只补 NULL 行，站长已手工指定的键不覆盖。

BEGIN;

UPDATE staff_panel_entries SET perm_key = 'user.view'
 WHERE tab_key = 'users' AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'user.view'
 WHERE tab_key = 'userlogs' AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'reports.handle'
 WHERE tab_key = 'reports' AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'cheaters.view'
 WHERE tab_key = 'cheaters' AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'subtitles.grant'
 WHERE tab_key = 'subawards' AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'categories.manage'
 WHERE tab_key IN ('cats', 'tagdict', 'sections') AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'settings.manage'
 WHERE tab_key IN ('menu', 'p2tools', 'freeleech') AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'jixiao.payroll.view'
 WHERE tab_key = 'jixiao' AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'user.resetpass'
 WHERE tab_key = 'resetpass' AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'roles.manage'
 WHERE tab_key = 'roles' AND perm_key IS NULL;
UPDATE staff_panel_entries SET perm_key = 'user.adjust'
 WHERE tab_key = 'applications' AND perm_key IS NULL;

-- 防呆：15 条都在册；少一条说明库里的 tab_key 已被改名，静默漏改就是
-- 下一个「点进去 403」。
DO $$
DECLARE
    n int;
BEGIN
    SELECT count(*) INTO n FROM staff_panel_entries
     WHERE tab_key IN ('users', 'userlogs', 'reports', 'cheaters',
                       'subawards', 'cats', 'tagdict', 'sections',
                       'menu', 'p2tools', 'freeleech', 'jixiao',
                       'resetpass', 'roles', 'applications');
    IF n <> 15 THEN
        RAISE EXCEPTION 'staff_panel_entries 预期 15 条对齐项，实际 % 条', n;
    END IF;
    SELECT count(*) INTO n FROM staff_panel_entries
     WHERE tab_key IN ('users', 'userlogs', 'reports', 'cheaters',
                       'subawards', 'cats', 'tagdict', 'sections',
                       'menu', 'p2tools', 'freeleech', 'jixiao',
                       'resetpass', 'roles', 'applications')
       AND perm_key IS NULL;
    IF n > 0 THEN
        RAISE EXCEPTION '仍有 % 条导航项没有权限键', n;
    END IF;
END $$;

COMMIT;
