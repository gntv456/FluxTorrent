-- 0144 登录页品牌区站型化（site_settings 键 + 类型包默认标语）
--
-- 背景：登录页标语「读书 · 学习 · 分享」此前硬编码在前端 i18n 字典——与站型
-- 无关且管理组不可配。影视站/游戏站套教育站标语明显错位。
--
-- 方案（两级：站型包默认 → site_settings 覆盖）：
--   * site_type_packs.tagline：每站型一句默认标语（apply 类型包时写入 site_settings.site_tagline）
--   * site_settings.site_tagline：站长后台可改，空值回落前端字典
--   * site_settings.site_logo：站点 logo 图片 URL（空 = 前端回落占位）
--
-- 全部幂等（ADD COLUMN IF NOT EXISTS / 守卫 UPDATE）。

ALTER TABLE site_type_packs ADD COLUMN IF NOT EXISTS tagline TEXT NOT NULL DEFAULT '';

-- 各站型默认标语（只在空值时填充，站长自定义不受影响）
UPDATE site_type_packs SET tagline = '读书 · 学习 · 分享'            WHERE code = 'education'   AND tagline = '';
UPDATE site_type_packs SET tagline = '光影 · 收藏 · 分享'            WHERE code = 'movie'       AND tagline = '';
UPDATE site_type_packs SET tagline = '聆听 · 分享 · 共鸣'            WHERE code = 'music'       AND tagline = '';
UPDATE site_type_packs SET tagline = '追番 · 补番 · 分享'            WHERE code = 'anime'       AND tagline = '';
UPDATE site_type_packs SET tagline = '阅读 · 求知 · 分享'            WHERE code = 'ebook'       AND tagline = '';
UPDATE site_type_packs SET tagline = '分享 · 互助 · 成长'            WHERE code = 'general'     AND tagline = '';
UPDATE site_type_packs SET tagline = '热爱 · 赛事 · 分享'            WHERE code = 'sports'      AND tagline = '';
UPDATE site_type_packs SET tagline = '游戏 · 闯关 · 分享'            WHERE code = 'game'        AND tagline = '';
UPDATE site_type_packs SET tagline = '工具 · 折腾 · 分享'            WHERE code = 'software'    AND tagline = '';
UPDATE site_type_packs SET tagline = '纪录 · 视野 · 分享'            WHERE code = 'documentary' AND tagline = '';
UPDATE site_type_packs SET tagline = '无损 · 品鉴 · 分享'            WHERE code = 'lossless'    AND tagline = '';

-- 存量站：按当前站型把包默认标语落进 site_settings（站长改过后即有值，不再覆盖）
INSERT INTO site_settings (name, value)
SELECT 'site_tagline', COALESCE(NULLIF(p.tagline, ''), '分享 · 互助 · 成长')
FROM site_type_packs p
WHERE p.code = COALESCE((SELECT value FROM site_settings WHERE name = 'site_type'), 'general')
ON CONFLICT (name) DO NOTHING;

-- logo 键占位（不插值：空 = 前端回落占位图形）
INSERT INTO site_settings (name, value) VALUES ('site_logo', '')
ON CONFLICT (name) DO NOTHING;

-- 管理面可配（0090 site_desc 同款注册模式）：
-- site_tagline 留空回落类型包默认；site_logo 支持绝对 URL 或 /attachments/<sha> 引用
INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, card_order, min_class) VALUES
('site_tagline', 'text', '登录页标语', 'Login tagline',
 '登录页品牌区的一句话标语；留空使用当前站型的默认标语（切换站型时自动更新）',
 '基础信息', 12, 99),
('site_logo', 'text', '站点 Logo', 'Site logo',
 '站点 logo 图片 URL（登录页品牌区/导航展示）；支持绝对 URL 或 /attachments/<sha>；留空显示占位图形',
 '基础信息', 13, 99)
ON CONFLICT (name) DO UPDATE SET
    label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
    hint = EXCLUDED.hint, group_key = EXCLUDED.group_key,
    card_order = EXCLUDED.card_order, min_class = EXCLUDED.min_class;

-- site_tagline 需要挂在 site_settings 才进 settings 编辑面（上面已 INSERT ... DO NOTHING）；
-- site_logo 同理。补 descr 列（0090 口径）供后台分组展示。
UPDATE site_settings SET descr = '登录页标语（留空使用站型默认）' WHERE name = 'site_tagline' AND (descr IS NULL OR descr = '');
UPDATE site_settings SET descr = '站点 Logo URL（留空占位图形）' WHERE name = 'site_logo' AND (descr IS NULL OR descr = '');
