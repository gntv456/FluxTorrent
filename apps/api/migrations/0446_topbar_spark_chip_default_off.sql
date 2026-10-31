-- 0446: 顶栏魔力 chip 缺省关（站长定调 2026-10-31：主菜单不显示魔力）。
--
-- topbar_spark_chip 此前缺省=显示（yes/缺省开）。定调主菜单走清爽路线：
-- 缺省翻转为关，想显示的站长在设置里显式开 yes。存量站若从未配置，
-- 落 no（与缺省一致）；显式配过 yes 的保留站长意志不动。
-- 余额查看不因此失去入口：头像弹窗摘要与 /my-spark 页仍在。

INSERT INTO site_settings (name, value, grp) VALUES
  ('topbar_spark_chip', 'no', 'appearance')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, options,
                           group_key)
VALUES
  ('topbar_spark_chip', 'yesno', '顶栏显示魔力余额',
   'Show spark balance chip in topbar',
   '开启后头像旁常驻一枚魔力余额胶囊（点击进我的火花）。'
   '默认关闭——主菜单保持清爽，余额在头像弹窗与「我的火花」页查看',
   '[{"v":"yes","l":"显示"},{"v":"no","l":"隐藏"}]'::jsonb,
   'appearance')
ON CONFLICT (name) DO UPDATE
  SET label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
      hint = EXCLUDED.hint, options = EXCLUDED.options,
      group_key = EXCLUDED.group_key;
