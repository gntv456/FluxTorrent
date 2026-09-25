-- 0192：站型工具导航条目（三审 B-1/B-2 方向盘补齐）。
--
-- 0186 用户字段 / 0187 自定义页面的后台端点此前无左侧导航入口——
-- 「有引擎没方向盘」。补 content 分区两条目（tab_key 与 staff-tools
-- ToolTab/STAFF_TOOL_TABS 三方一致：userfields / pages）。

INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key)
SELECT * FROM (VALUES
    ('admin', '自定义字段', '/admin?tool=userfields',
     '定义用户资料自定义字段（文本/单选/注册页展示位）', 9, 'content', 93, 'userfields'),
    ('admin', '自定义页面', '/admin?tool=pages',
     '创建任意内容页（/p/slug），可挂接自定义菜单', 10, 'content', 93, 'pages')
) AS v(panel, name, url, info, sort, section, min_class, tab_key)
WHERE NOT EXISTS (
    SELECT 1 FROM staff_panel_entries e
    WHERE e.tab_key IN ('userfields', 'pages'));
