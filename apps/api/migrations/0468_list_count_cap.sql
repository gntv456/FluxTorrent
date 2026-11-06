-- 0468 种子列表分页增强：页码跳转（OFFSET 模式）+ 计数封顶可配
-- 背景（好学站迁移后用户反馈）：①游标翻页不能跳页、无总页数；
-- ②total_estimate 采样封顶 10001，2.3 万种的站显示「约 10001」。
-- API：?page>1 走 OFFSET 翻页；count 封顶读本键（<=0 = 精确全量 count，
-- 默认 10001 保持历史口径——大站可调回采样护 DB）。
INSERT INTO site_settings (name, value)
VALUES ('list_count_cap', '10001')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, unit, min, max, step, group_key, card_order) VALUES
('list_count_cap', 'number', '列表计数封顶', 'List count cap', '条', 0, 1000000, 1, '页面分页', 10)
ON CONFLICT (name) DO NOTHING;
