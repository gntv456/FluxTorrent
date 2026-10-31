-- 0445: 站点 / 登录页背景图（外观与主题）：URL + 不透明度，站长可配。
--
-- 两个 URL 默认空（不启用）；不透明度 0~100（越大背景图越清晰）。
-- /site-profile 以 backgrounds:{site:{url,opacity}, login:{url,opacity}}
-- 下发（staff_http/sitetype.rs）：站点页在 (main) 布局铺固定底层，
-- 登录/注册画布铺左栏背景；内容卡片保持在上层。

INSERT INTO site_settings (name, value, grp) VALUES
  ('site_bg_url', '', 'appearance'),
  ('site_bg_opacity', '25', 'appearance'),
  ('login_bg_url', '', 'appearance'),
  ('login_bg_opacity', '50', 'appearance')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, min, max, step, options,
   group_key)
VALUES
  ('site_bg_url', 'text', '站点背景图', 'Site background',
   '全站页面背景图的绝对地址（http/https）；留空 = 不启用。'
   '图片铺在内容之下，卡片与文字保持可读',
   '', NULL, NULL, NULL, '{"rule":"url"}'::jsonb, 'appearance'),
  ('site_bg_opacity', 'number', '站点背景图不透明度',
   'Site background opacity',
   '0~100：越大背景图越清晰，越小越淡（建议 15~40）',
   '%', 0, 100, 5, NULL, 'appearance'),
  ('login_bg_url', 'text', '登录页背景图', 'Login background',
   '登录页左栏品牌区的背景图绝对地址（http/https）；留空 = 沿用现有渐变',
   '', NULL, NULL, NULL, '{"rule":"url"}'::jsonb, 'appearance'),
  ('login_bg_opacity', 'number', '登录页背景图不透明度',
   'Login background opacity',
   '0~100：越大背景图越清晰，越小越淡（建议 40~70，选浅色图）',
   '%', 0, 100, 5, NULL, 'appearance')
ON CONFLICT (name) DO NOTHING;

-- grp 必须显式归 appearance：缺省落 misc，站长会在「其他设定」里找不到
UPDATE site_settings SET grp = 'appearance'
 WHERE name IN ('site_bg_url','site_bg_opacity','login_bg_url','login_bg_opacity')
   AND grp IS DISTINCT FROM 'appearance';
