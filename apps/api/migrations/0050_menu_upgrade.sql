-- 自定义菜单升级（xiaomlove/nexusphp-menu 生产口径，源码与数据已存档至 hxpt 仓库 _doc/）：
-- 1) menu_items 增加树形 parent_id / 打开方式 target / 最低可见等级 min_class
-- 2) site_settings 增加全局开关 nav.custom_enabled 与 nav.min_visible_class
--    （生产为 menu.enabled / menu.minimum_visible_class，键名按 FluxTorrent 命名习惯归到 nav.*）

ALTER TABLE menu_items
  ADD COLUMN IF NOT EXISTS parent_id BIGINT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS target TEXT NOT NULL DEFAULT '_self',
  ADD COLUMN IF NOT EXISTS min_class INT NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_menu_items_parent ON menu_items (parent_id);

INSERT INTO site_settings (name, value, descr, grp) VALUES
  ('nav.custom_enabled', '0', '启用自定义导航：开启且配置了顶栏菜单项后替换默认一级导航', 'basic'),
  ('nav.min_visible_class', '0', '自定义导航整体最低可见等级（0=所有人）', 'basic')
ON CONFLICT (name) DO NOTHING;
