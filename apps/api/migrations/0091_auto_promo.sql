-- 0089: 发种自动促销（NP 促销设置 口径）+ 二级置顶
--
-- 1) 自动促销：管理后台配置默认促销（类型 + 天数），发布即套用，
--    发布者不再在发布页单独设置促销（促销跟随站点）。
-- 2) 置顶扩展：pos_state 支持 1=一级置顶、2=二级置顶（列表排序二级低于一级），
--    发布/批量工作台均可用；截止时间沿用 pos_state_until。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('upload_auto_promo_kind', '', '发种自动促销类型（空=关闭）：free/x2/x2free/half/x2half/p30', 'basic')
ON CONFLICT (name) DO NOTHING;

INSERT INTO site_settings (name, value, descr, grp) VALUES
('upload_auto_promo_days', '0', '发种自动促销天数（天，0=跟随类型关闭）', 'basic')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, card_order, min_class) VALUES
('upload_auto_promo_kind', 'text', '发种自动促销类型', 'Auto promo kind', '新发布种子自动套用的促销类型：free=免费 x2=双倍 x2free=双倍+免费 half=半价 x2half=双倍+半价 p30=30%下载；留空关闭', '经济', 21, 99)
ON CONFLICT (name) DO UPDATE SET
    label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
    hint = EXCLUDED.hint, group_key = EXCLUDED.group_key,
    card_order = EXCLUDED.card_order, min_class = EXCLUDED.min_class;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, card_order, min_class) VALUES
('upload_auto_promo_days', 'number', '发种自动促销天数', 'Auto promo days', '自动促销持续天数（1-720）；类型为空时本项无效', '经济', 22, 99)
ON CONFLICT (name) DO UPDATE SET
    label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
    hint = EXCLUDED.hint, group_key = EXCLUDED.group_key,
    card_order = EXCLUDED.card_order, min_class = EXCLUDED.min_class;
