-- 0393_forum_nav_label.sql
-- 论坛管理导航条目改名（对齐样图「后台 / 论坛管理」面包屑与页标题）。
--
-- 背景：论坛管理已升格为独立整页 /admin/forums（左分区树 + 右版块表），
-- 但 staff_panel_entries 里该条目的种子名还是旧值「论坛版块」——
-- 前台导航文案以 i18n adminNav.forums 为准（已改「论坛管理」），DB 值
-- 只在 i18n 缺失时兜底，并在 panelentries 工具里原样展示。两边不一致
-- 会让站长在条目编辑器里看到与导航不同的旧名，故同步种子值。
--
-- 纯 UPDATE、幂等；不改权限/排序/URL，行为零变化。

UPDATE staff_panel_entries
   SET name = '论坛管理',
       info = '分区与版块的增删改排 · 版主任免'
 WHERE tab_key = 'forums';
