-- 0235_gacha_admin_panel.sql — G31-C 尾：后台「抽卡运营」面板入口
--
-- staff_panel_entries 挂 tool 页签（admin 面板 users 组，user.adjust 权限）：
-- 发券/发碎片/池参数/卡定义 CRUD 走这一个入口（面板本体是前端
-- admin-gacha 组件；本迁移只落导航行，让 ?tool=gacha 不再落 panelEmpty）。
-- module_key=gacha：模块关闭时入口随网关与 nav 三处同时消失（0229 口径）。

INSERT INTO staff_panel_entries
    (id, panel, name, url, info, sort, section, min_class, tab_key,
     module_key, perm_key)
VALUES
    (106, 'admin', '抽卡运营', '/admin?tool=gacha',
     '发券/发碎片、卡池与卡定义、抽取流水', 64, 'users', 92, 'gacha',
     'gacha', 'user.adjust')
ON CONFLICT (id) DO UPDATE
    SET name = EXCLUDED.name, url = EXCLUDED.url, info = EXCLUDED.info,
        sort = EXCLUDED.sort, section = EXCLUDED.section,
        tab_key = EXCLUDED.tab_key, module_key = EXCLUDED.module_key,
        perm_key = EXCLUDED.perm_key;
