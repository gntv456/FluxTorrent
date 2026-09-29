-- 娱乐屋运营面板（staff 只读）导航注册（2026-09-29）
-- 面板本体是前端 admin-arcade 组件 + 后端 GET /admin/arcade/overview；
-- 本迁移只落导航行，让 ?tool=arcade 不再落 panelEmpty（0235 gacha 同款口径）。
-- module_key=games：模块关闭时入口随网关与 nav 三处同时消失（0229 口径）。
-- perm_key=user.adjust：与 gacha 运营面板同级权限。

INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key,
     module_key, perm_key)
VALUES
    ('admin', '娱乐屋运营', '/admin?tool=arcade',
     '三口径 EV 对照 / 门禁自检 / 参数现值 / 定义一览', 65, 'ops', 92,
     'arcade', 'games', 'user.adjust')
ON CONFLICT DO NOTHING;
