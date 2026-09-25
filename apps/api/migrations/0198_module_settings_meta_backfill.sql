-- 0198：给「后加的模块键」补设置页开关（四审 L5 P1 收尾）
--
-- 0107 用派生插入把 modules 表每个键映射成一对行：site_settings 的 `module_<key>` 值行
-- + settings_meta 的 yesno 登记行。但那是历史快照：之后新注册的键（0197 的 invites）
-- 两边都没有，结果是「注册表里有这个模块、后台找不到开关」，
-- PUT /admin/settings/groups 还会以「未知设定项」拒写。
--
-- 顺序不能反：settings_meta.name 有 FK 指向 site_settings(name)，先插值行再插登记行。
-- 只补缺、不改已有（NOT EXISTS / ON CONFLICT DO NOTHING）：已存在的 29 个开关可能已被
-- 站长改过值或标签，不能被派生语句覆盖回去。

-- 1) 值行：跟随 modules.is_on 的当前真值
INSERT INTO site_settings (name, value, descr, grp)
SELECT 'module_' || m.key,
       CASE WHEN m.is_on THEN 'yes' ELSE 'no' END,
       m.name_zh || '（模块开关）',
       'module'
FROM modules m
ON CONFLICT (name) DO NOTHING;

-- 2) 登记行：后台设置中心「模块开关」卡片（分组沿用 0107 的 module_<grp> 口径）
INSERT INTO settings_meta (name, type, label_zh, label_en, group_key, card_order)
SELECT 'module_' || m.key,
       'yesno',
       m.name_zh || '（' || m.descr || '）',
       m.name_en,
       'module_' || m.grp,
       10 + (row_number() OVER (ORDER BY m.grp, m.key))::int
FROM modules m
WHERE NOT EXISTS (
    SELECT 1 FROM settings_meta s WHERE s.name = 'module_' || m.key
);
