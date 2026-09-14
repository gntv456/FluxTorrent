-- 0082: 站点货币自定义名称（火花 → 默认「魔力」，站长可在后台改任何名字）
--
-- 前端全站文案（导航/商店/银行/游戏/签到/账单…）从 site-profile 读取 currency_name
-- 动态渲染；后端管理端点/校验文案中的「火花」同步引用该设定。
-- 存量站：不预设旧名「火花」，统一落到默认「魔力」（默认即目标口径）。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('currency_name', '魔力', '站点货币名称（全站文案显示用）', 'basic')
ON CONFLICT (name) DO UPDATE SET descr = EXCLUDED.descr;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, card_order, min_class) VALUES
('currency_name', 'text', '站点货币名称', 'Currency name', '全站显示的货币名（如：魔力 / 火花 / 猫粮），2-12 个字符', '基础信息', 9, 99)
ON CONFLICT (name) DO UPDATE SET
    label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
    hint = EXCLUDED.hint, group_key = EXCLUDED.group_key,
    card_order = EXCLUDED.card_order, min_class = EXCLUDED.min_class;

-- 长度护栏：>12 字符截断（validate_field 的 text 规则不拦长度下限，此处兜底）
UPDATE site_settings SET value = left(value, 12) WHERE name = 'currency_name' AND length(value) > 12;
