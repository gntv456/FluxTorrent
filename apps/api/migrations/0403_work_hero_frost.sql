-- 作品详情页 Hero 磨砂底图（0403）：海报高斯模糊 + 遮罩深度，站长可调。
-- 两个键都属「外观与主题」分区（settings_meta.group_key='appearance'），
-- 站点档案 /site-profile 以 work_hero:{blur,dim} 下发（staff_http/sitetype.rs）。

-- 默认值：模糊 28px、遮罩 0.55（0=不遮罩，1=全遮）
-- grp='appearance'（外观与主题）：后台分区按 site_settings.grp 归，
-- 缺省是 misc（其他设定）——两个磨砂参数是外观项，必须显式归组
INSERT INTO site_settings (name, value, grp) VALUES
  ('work_hero_blur', '28', 'appearance'),
  ('work_hero_dim', '0.55', 'appearance')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, min, max, step, group_key) VALUES
  ('work_hero_blur', 'number', '作品页海报磨砂模糊', 'Work hero blur',
   '作品详情页 Hero 用海报做磨砂底图的模糊半径，单位 px；0 = 不模糊',
   'px', 0, 80, 1, 'appearance'),
  ('work_hero_dim', 'number', '作品页海报遮罩深度', 'Work hero dim',
   '磨砂底图上再压一层深色遮罩的深度，0~0.9；越大越暗、文字越清晰',
   '', 0, 0.9, 0.05, 'appearance')
ON CONFLICT (name) DO NOTHING;

UPDATE site_settings SET grp = 'appearance'
 WHERE name IN ('work_hero_blur', 'work_hero_dim') AND grp IS DISTINCT FROM 'appearance';
