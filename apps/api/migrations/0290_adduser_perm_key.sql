-- 0290：补齐 adduser 面板条目的权限键（0209 漏网之鱼）
--
-- 现状：「添加用户」菜单项 min_class=93 且无 perm_key（0209 回填时遗漏），
-- 导致总版主(93)/管理员(94)/主管(95) 能看到入口，但 POST /admin/adduser
-- 的 require_perm(user.create) 只放行站长(99) 档——菜单可见而调用 403。
-- 修法：绑 perm_key='user.create'，让导航过滤（user_can）与端点守卫同口径。

UPDATE staff_panel_entries SET perm_key = 'user.create'
WHERE tab_key = 'adduser' AND perm_key IS NULL;
