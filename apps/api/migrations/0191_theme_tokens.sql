-- 0191：主题令牌键（一审 R4.6）——站长后台可改品牌主色等 CSS 变量。
--
-- 仅开放日间主题的品牌八色（Aurora 六色 + 品牌辉光/彩带底色），不开放
-- 表面/文字等中性变量（可读性安全线）。前端 layout.tsx 把有值的键注入
-- <style>:root 覆盖（data-theme 任意主题下生效）；theme 内容包（pack_format
-- THEME_KEYS）同步扩展同一批键——令牌随包分发。

-- settings_meta.name 有 FK → site_settings（先萧行再登记元数据）
INSERT INTO site_settings (name, value, descr, grp)
SELECT name, '', label_zh, 'appearance'
FROM (VALUES
('theme_token_sky',     '主题色·天蓝'),
('theme_token_sun',     '主题色·暖阳'),
('theme_token_coral',   '主题色·珊瑚'),
('theme_token_mint',    '主题色·薄荷'),
('theme_token_candy',   '主题色·糖果'),
('theme_token_indigo',  '主题色·靛蓝'),
('theme_token_glow',    '品牌辉光色'),
('theme_token_ribbon',  '彩带底色')
) AS t(name, label_zh)
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, group_key, card_order, hint) VALUES
('theme_token_sky',     'text', '主题色·天蓝', 'Theme·Sky',     'appearance', 61, '#rrggbb，空=默认 #2fa8ff'),
('theme_token_sun',     'text', '主题色·暖阳', 'Theme·Sun',     'appearance', 62, '#rrggbb，空=默认 #ffc93c'),
('theme_token_coral',   'text', '主题色·珊瑚', 'Theme·Coral',   'appearance', 63, '#rrggbb，空=默认 #ff7a59'),
('theme_token_mint',    'text', '主题色·薄荷', 'Theme·Mint',    'appearance', 64, '#rrggbb，空=默认 #2fbf9b'),
('theme_token_candy',   'text', '主题色·糖果', 'Theme·Candy',   'appearance', 65, '#rrggbb，空=默认 #ff8fc7'),
('theme_token_indigo',  'text', '主题色·靛蓝', 'Theme·Indigo',  'appearance', 66, '#rrggbb，空=默认 #5b6bf5'),
('theme_token_glow',    'text', '品牌辉光色',  'Brand Glow',    'appearance', 67, '#rrggbb，空=默认 #5b6bf5'),
('theme_token_ribbon',  'text', '彩带底色',    'Ribbon Base',   'appearance', 68, '#rrggbb，空=默认渐变（填纯色时替换渐变底）')
ON CONFLICT (name) DO NOTHING;
