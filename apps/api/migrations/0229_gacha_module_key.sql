-- 0229_gacha_module_key.sql — G31-A 模块键八处的库侧（方案 §4 地点 5/6/7）
--
-- 顺序铁律（0197 教训）：site_settings 值行**必须先于** settings_meta 登记行——
-- settings_meta.name FK 指向 site_settings.name，反了后台找不到开关、
-- PUT /admin/settings/groups 以「未知设定项」拒写（0198 才补的坑）。
-- gacha 缺省开（default_on 白名单外；公示面只读无经济行为，与兄弟模块 games 同口径），
-- 12 个站型包快照各补 `"gacha":true`（0107 先例：modules 是 JSONB 对象）。

-- 注册表行：modules 只有 key/name_zh/name_en/descr/grp 五列（真实 schema 实测），
-- 缺省开/关不在表上——在 default_on 白名单（gacha 不在关清单 → 缺省开）+ 值行。
INSERT INTO modules (key, name_zh, name_en, descr, grp) VALUES
('gacha', '抽卡屋', 'Gacha',
 '收藏卡抽卡与公示（娱乐屋延伸；概率按含保底综合口径公示）', 'fun')
ON CONFLICT (key) DO NOTHING;

-- 值行（先）：gacha 缺省开 → 'yes'（0107 值口径 yes/no，非 1/0）
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('module_gacha', 'yes', '抽卡屋（模块开关）', 'module')
ON CONFLICT (name) DO NOTHING;

-- 登记行（后）：card_order 让到 module_fun 组现有最大值之后，不与兄弟撞座
INSERT INTO settings_meta (name, type, label_zh, label_en, group_key, card_order)
SELECT 'module_' || key, 'yesno', name_zh || '（' || descr || '）', name_en,
       'module_' || grp,
       11 + COALESCE((SELECT max(card_order) FROM settings_meta
                      WHERE group_key = 'module_' || grp), 0)
FROM modules
WHERE key = 'gacha'
ON CONFLICT (name) DO UPDATE
  SET label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
      group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;

-- 站型包快照：所有未含该键的包补 gacha:true（新站型包由 apply 台账自带，0223 先例）
UPDATE site_type_packs
SET modules = COALESCE(modules::jsonb, '{}'::jsonb) || '{"gacha":true}'::jsonb
WHERE modules IS NULL OR NOT modules::jsonb ? 'gacha';
