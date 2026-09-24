-- 0168: 生态商店 M2 —— 商店索引设置键（策划案 §7 / §8 M2）
--
-- M1 是纯本地导入导出；M2 给后台内容包页加「目录」：
--   1) 内置目录：随核心发版的站型分类学包（site_type_packs.categories/sections
--      动态生成，不物化——0145 口径），离线可用；
--   2) 远程索引：站长可配 marketplace_index_url（JSON 索引，见策划案 §7.1），
--      拉取失败静默降级为「仅内置」（商店是增强不是依赖）。
-- 本迁移只落一个键 + 元数据登记；目录生成逻辑在 Rust 侧 pack_catalog.rs。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('marketplace_index_url', '', '生态商店远程索引地址（留空 = 仅内置目录）', 'module')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, group_key, card_order) VALUES
('marketplace_index_url', 'text', '生态商店远程索引地址（留空 = 仅内置目录）', 'Marketplace index URL', 'module', 95)
ON CONFLICT (name) DO UPDATE
  SET label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
      group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;
