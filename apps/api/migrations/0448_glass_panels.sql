-- 0448: 板块磨砂玻璃（iOS 风格）：不透明度 + 模糊半径两档。
--
-- 前端把 --surface-card（全站板块的表面令牌，--card/--panel/--surface/
-- --compat-paper/--field-bg 都是它的别名）在 .site-shell 上换成带 alpha 的
-- 同色，再给板块清单挂 backdrop-filter，于是「一个旋钮罩全站板块」。
-- glass_alpha=100 时前端整套规则直接不启用（不挂 backdrop-filter，省 GPU）。
-- 与 0445/0447 的背景图同组（appearance），/site-profile 以 glass 下发。

INSERT INTO site_settings (name, value, grp) VALUES
  ('glass_alpha', '88', 'appearance'),
  ('glass_blur', '12', 'appearance')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, min, max, step, options,
   group_key)
VALUES
  ('glass_alpha', 'number', '板块不透明度', 'Panel opacity',
   '0~100：越小越透明（磨砂越明显）。100 = 关闭玻璃；建议 85~92，'
   '低于 70 时正文对比度会开始吃紧',
   '%', 0, 100, 1, NULL, 'appearance'),
  ('glass_blur', 'number', '板块磨砂强度', 'Panel blur radius',
   '0~30px：背景透过板块后的模糊半径，越大越像磨砂玻璃；'
   '不支持 backdrop-filter 的浏览器自动回退为不透明板块',
   'px', 0, 30, 1, NULL, 'appearance')
ON CONFLICT (name) DO NOTHING;

UPDATE site_settings SET grp = 'appearance'
 WHERE name IN ('glass_alpha','glass_blur')
   AND grp IS DISTINCT FROM 'appearance';
