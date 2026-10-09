-- 0339_staff_panel_trumps.sql
-- 管理台导航：举报处理（0336 trumping 的版主入口）。
--
-- 背景：0336 落了 `torrent_trumps` 表与三个端点（举报 / 队列 / 裁决），但
-- **管理台没有入口**——版主看不到待处理队列，能力等于空转。本迁移只注册
-- 导航项。⚠️ `tab_key` 必须与前端 `renderSimpleTool` 的 case、i18n 三处一致，
-- 改完跑 `_admin_nav_align.py` 对差集（见 admin-tool-switch.tsx 顶部注记）。
--
-- 幂等：WHERE NOT EXISTS。

BEGIN;

INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key,
     module_key, perm_key)
SELECT 'moderator', '举报处理', '/admin?tool=trumps',
       '处理用户举报的劣质/重复种子（trumping 裁决）',
       5, 'content', 90, 'trumps', '', 'torrent.manage'
 WHERE NOT EXISTS (
     SELECT 1 FROM staff_panel_entries WHERE tab_key = 'trumps'
 );

COMMIT;
