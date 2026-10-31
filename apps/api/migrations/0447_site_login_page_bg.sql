-- 0447: 登录页主背景（整页，画布之外）：URL + 不透明度。
--
-- 0445 的 login_bg_* 铺在登录画布**左栏品牌区**；站长还需要整页主背景。
-- 新增 login_page_bg_url / login_page_bg_opacity（外观与主题），
-- /site-profile 以 backgrounds.loginPage 下发；空 URL = 不启用。
-- 顺手把 0445 两个键的标签明确为「左栏」，避免后台两处混淆。

INSERT INTO site_settings (name, value, grp) VALUES
  ('login_page_bg_url', '', 'appearance'),
  ('login_page_bg_opacity', '40', 'appearance')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, min, max, step, options,
   group_key)
VALUES
  ('login_page_bg_url', 'text', '登录页主背景图',
   'Login page background',
   '登录画布之外的整页背景图绝对地址（http/https）；留空 = 沿用现有渐变',
   '', NULL, NULL, NULL, '{"rule":"url"}'::jsonb, 'appearance'),
  ('login_page_bg_opacity', 'number', '登录页主背景不透明度',
   'Login page background opacity',
   '0~100：越大背景图越清晰，越小越淡（建议 25~50）',
   '%', 0, 100, 5, NULL, 'appearance')
ON CONFLICT (name) DO NOTHING;

-- 0445 的两项只作用于左栏：标签点名「左栏」，与主背景区分
UPDATE settings_meta
   SET label_zh = '登录页左栏背景图',
       label_en = 'Login hero background'
 WHERE name = 'login_bg_url';
UPDATE settings_meta
   SET label_zh = '登录页左栏背景不透明度',
       label_en = 'Login hero background opacity'
 WHERE name = 'login_bg_opacity';

UPDATE site_settings SET grp = 'appearance'
 WHERE name IN ('login_page_bg_url','login_page_bg_opacity')
   AND grp IS DISTINCT FROM 'appearance';
