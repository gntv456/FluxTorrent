-- 0421: 三个「面板已实现、后台导航里没有」的入口补上（2026-10-25 版主八轮实测）。
--
-- 现状：admin-tool-switch.tsx 的 renderSimpleTool 里有 case，前端也有面板组件，
-- 后端读口/写口都在，但 staff_panel_entries 里没有对应的 tab_key ——
-- 只能手敲 /admin?tool=ops 才进得去，后台「职能导航」与 Ctrl/⌘+K 搜索都找不到它。
-- 同域的先例都是成对交付（0402 给 autoreview 补导航、0339 给 trumps 补导航），
-- 这三条是漏写，不是刻意隐藏：
--   * ops        → AdminOpsPanel（0078 运维三件套：版本信息 / 备份面板 / 任务手动触发）
--                  读 /admin/version(staff.panel 90)
--                    + /admin/backups(settings.manage 99) + /admin/jobs(98)
--   * homelayout → HomeLayoutEditor（0089 首页排版：板块顺序/宽度/显隐）
--                  读 /home(登录) + 写 PUT /admin/home-layout(settings.manage 99)
--   * viewlayout → ViewHiddenEditor（E6 视图布局：列表列与详情段落显隐）
--                  读 /view-layout(登录) + /site-profile
--                    + 写 PUT /admin/view-hidden(99)
-- 三条的**首屏写口与最紧的读口都是 settings.manage（现网仅 class/99）**，
-- 于是 perm_key 就钉这一把键、min_class 同 99：口径与 0420 那条轴一致
-- （外露必须打得开，藏起来的必须是打不开）。
-- 幂等：tab_key 上有部分唯一索引，重复执行是空操作。

BEGIN;

INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key, perm_key)
SELECT 'sysop', '运维面板', '/admin?tool=ops',
       '版本信息、备份面板与任务手动触发', 17, 'system', 99, 'ops',
       'settings.manage'
 WHERE NOT EXISTS (SELECT 1 FROM staff_panel_entries WHERE tab_key = 'ops');

INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key, perm_key)
SELECT 'sysop', '首页排版', '/admin?tool=homelayout',
       '首页板块顺序、宽度与显隐可视化编辑', 13, 'content', 99, 'homelayout',
       'settings.manage'
 WHERE NOT EXISTS (
     SELECT 1 FROM staff_panel_entries WHERE tab_key = 'homelayout');

INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key, perm_key)
SELECT 'sysop', '视图布局', '/admin?tool=viewlayout',
       '种子列表列与详情页段落的站点级显隐', 14, 'content', 99, 'viewlayout',
       'settings.manage'
 WHERE NOT EXISTS (SELECT 1 FROM staff_panel_entries WHERE tab_key = 'viewlayout');

-- 防呆：三条都在册，且都带 settings.manage；少一条或键漂了就炸。
DO $$
DECLARE
    n int;
BEGIN
    SELECT count(*) INTO n FROM staff_panel_entries
     WHERE tab_key IN ('ops', 'homelayout', 'viewlayout');
    IF n <> 3 THEN
        RAISE EXCEPTION 'staff_panel_entries 预期补齐 3 条导航，实际 % 条', n;
    END IF;
    SELECT count(*) INTO n FROM staff_panel_entries
     WHERE tab_key IN ('ops', 'homelayout', 'viewlayout')
       AND (perm_key IS NULL OR perm_key <> 'settings.manage'
            OR min_class <> 99);
    IF n > 0 THEN
        RAISE EXCEPTION '% 条新导航的权限档与首屏后端不一致', n;
    END IF;
END $$;

COMMIT;
