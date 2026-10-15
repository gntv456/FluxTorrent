-- 0373 M4/M5 后台导航补录（emaildomains / regquiz）
--
-- M4（0370）落了四道注册闸与一次性邮箱域名库 API，M5（0372）落了答题题库
-- API，但都没有后台入口——`?tool=X` 能不能渲染取决于三源对齐
-- （DB staff_panel_entries.tab_key / web ToolTab 联合类型 / STAFF_TOOL_TABS），
-- 缺 DB 行则导航里看不见（深链仍可进），缺 web 则静默落进概览页。
-- 本迁移补 DB 侧两行；幂等（tab_key 已存在则跳过）。
--
-- section 取 system（与 bans/testip/agentrules 同域）；sort 取 15/16
-- （system 域现用 0..14，避免与 trackers(14) 撞序）。

INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key,
     perm_key)
SELECT 'admin', '一次性邮箱域名', '/admin?tool=emaildomains',
       '注册准入：拦截一次性/临时邮箱域名（内置库 + 导入导出）',
       15, 'system', 90, 'emaildomains', 'settings.manage'
WHERE NOT EXISTS (
    SELECT 1 FROM staff_panel_entries WHERE tab_key = 'emaildomains'
);

INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key,
     perm_key)
SELECT 'admin', '注册答题题库', '/admin?tool=regquiz',
       '注册闸：题库 CRUD / 判分票据 / 近 N 天通过率',
       16, 'system', 90, 'regquiz', 'settings.manage'
WHERE NOT EXISTS (
    SELECT 1 FROM staff_panel_entries WHERE tab_key = 'regquiz'
);

COMMIT;
