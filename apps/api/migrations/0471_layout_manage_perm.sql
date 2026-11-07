-- 0471: 排版编辑拆细键 layout.manage，授维护开发员档（98）。
--
-- 现状：PUT /admin/home-layout 与 PUT /admin/view-hidden 都要
--   settings.manage（0054 种子仅 class/99）。而前端 admin-shell 对全体
--   职员无条件补注「首页排版 / 视图布局」两条导航 → 98 档维护开发员
--   看得到入口、打得开编辑器，点保存却 403（入口与执法不同钥匙）。
--   且 0058 已把 sitepacks.manage 授给 98 档，站型包 apply 会连带写
--   home_layout（pack_core）——间接路径改得了排版，直接编辑器却锁 99，
--   两条路径不一致。
-- 处理：照 0217/0359 细键范式拆 layout.manage：
--   * 只管首页排版 + 视图布局两个站点级展示配置，不连带解锁
--     settings.manage 的其余站点设定（备份/促销/菜单/邮件域名等仍 99）。
--   * 新授 class/98（维护开发员及以上）；settings.manage 的既有持有者
--     （档/职务/用户级覆盖）原样继承，零漂移。
--   * 两条导航 perm_key/min_class 同步改同键同档，恢复「外露必须打得开」
--     （0421 轴）；前端补注亦按 min_class 收口。

BEGIN;

-- ============ 1) 细键登记 ============
INSERT INTO permissions (key, name, category, descr, sort, implemented)
VALUES ('layout.manage', '页面排版', 'content',
        '首页板块排版与视图布局（列表列/详情段落显隐）的站点级编辑', 55, true)
ON CONFLICT (key) DO NOTHING;

-- ============ 2) 授权继承（零漂移） ============
-- 档/职务：凡持有 settings.manage 的原样复制 granted
-- （含显式拒绝行——避免把「拒绝」洗成「未授」，0217 口径）。
INSERT INTO role_permissions (role_type, role_key, permission_key, granted)
SELECT rp.role_type, rp.role_key, 'layout.manage', rp.granted
FROM role_permissions rp
WHERE rp.permission_key = 'settings.manage'
ON CONFLICT DO NOTHING;

-- 用户级覆盖同继承（否则被显式授过 settings.manage 的用户换键后失权）。
INSERT INTO user_permissions
    (user_id, permission_key, granted, granted_by, note)
SELECT up.user_id, 'layout.manage', up.granted, up.granted_by, up.note
FROM user_permissions up
WHERE up.permission_key = 'settings.manage'
ON CONFLICT (user_id, permission_key) DO NOTHING;

-- 新授：维护开发员档（98）及以上。0058 定义 98 档为「系统运维能力」，
-- 排版编辑属日常运维，不是 99 档的用户管理高危操作。
INSERT INTO role_permissions (role_type, role_key, permission_key)
VALUES ('class', '98', 'layout.manage')
ON CONFLICT DO NOTHING;

-- ============ 3) 导航条目同键同档（条目被后台删过则补回，0421 形态） ============
-- section 归一到 system：前端 NAV_SUBGROUPS.system.sysCustom 登记了这两键，
-- 0421 手写 content 会让条目落进「内容」组的 loose 平铺（与注入形不一致）。
INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key, perm_key)
SELECT 'sysop', '首页排版', '/admin?tool=homelayout',
       '首页板块顺序、宽度与显隐可视化编辑', 13, 'system', 98,
       'homelayout', 'layout.manage'
 WHERE NOT EXISTS (
     SELECT 1 FROM staff_panel_entries WHERE tab_key = 'homelayout');

INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key, perm_key)
SELECT 'sysop', '视图布局', '/admin?tool=viewlayout',
       '种子列表列与详情页段落的站点级显隐', 14, 'system', 98,
       'viewlayout', 'layout.manage'
 WHERE NOT EXISTS (
     SELECT 1 FROM staff_panel_entries WHERE tab_key = 'viewlayout');

UPDATE staff_panel_entries
   SET perm_key = 'layout.manage', min_class = 98, section = 'system'
 WHERE tab_key IN ('homelayout', 'viewlayout');

-- 防呆：两条都在册且带新键新档；少一条或键漂了就炸。
DO $$
DECLARE
    n int;
BEGIN
    SELECT count(*) INTO n FROM staff_panel_entries
     WHERE tab_key IN ('homelayout', 'viewlayout')
       AND perm_key = 'layout.manage' AND min_class = 98;
    IF n <> 2 THEN
        RAISE EXCEPTION 'layout.manage 导航收口预期 2 条，实际 % 条', n;
    END IF;
END $$;

COMMIT;