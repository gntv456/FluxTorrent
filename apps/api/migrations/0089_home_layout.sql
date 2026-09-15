-- 0089: 首页板块自定义排版。
--
-- site_settings.home_layout 存 JSON 数组，每项 {key, span}：
--   key  = 板块键（news/attendance/shoutbox/funbox/resource_stats/site_data/
--          lucky_draw/links/latest），顺序即渲染顺序；
--   span = 宽度档（1/2/3 = 1/3、2/3、整行；未写默认按板块推荐档）。
-- 未配置（值空/非法）→ 前端回退内置默认排版（即现有布局），存量站零影响。
-- 管理端编辑入口走 settings_meta 的 textarea 字段（JSON 校验由端点执行）。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('home_layout', '', '首页板块排版（JSON 数组，空 = 默认布局）', 'main')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, card_order, min_class, visible) VALUES
('home_layout', 'textarea', '首页板块排版', 'Home layout',
 'JSON 数组，如 [{"key":"news","span":2},{"key":"attendance","span":1}]；可用键：news/attendance/shoutbox/funbox/resource_stats/site_data/lucky_draw/links/latest；span 取 1/2/3（1/3、2/3、整行宽度）；留空 = 默认排版',
 '外观', 5, 99, true)
ON CONFLICT (name) DO UPDATE SET
    label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
    hint = EXCLUDED.hint, group_key = EXCLUDED.group_key,
    card_order = EXCLUDED.card_order, min_class = EXCLUDED.min_class;
