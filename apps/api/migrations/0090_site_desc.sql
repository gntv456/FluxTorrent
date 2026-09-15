-- 0088: 站点简介可配置（页脚「站点信息」卡片文案）
-- 站长在后台基础信息里改 site_desc；缺省回落 i18n 字典文案（教育站默认语，兼容存量站）。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('site_desc', '', '站点简介（页脚「站点信息」展示；留空使用默认文案）', 'basic')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, card_order, min_class) VALUES
('site_desc', 'text', '站点简介', 'Site description', '页脚「站点信息」卡片的一句话简介；留空显示默认文案', '基础信息', 11, 99)
ON CONFLICT (name) DO UPDATE SET
    label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
    hint = EXCLUDED.hint, group_key = EXCLUDED.group_key,
    card_order = EXCLUDED.card_order, min_class = EXCLUDED.min_class;
