-- 0160_tag_groups.sql — 标签体系 P2：分组 + 通用层（策划案 §4.1/裁决 5）
-- 1) tag_group：attribute=属性类（首发/官种/禁转/国语/中字/DIY/完结…）
--    content=内容类（合集/带答案/题材…）——发布/筛选/详情按组分区渲染
-- 2) scope_layer：global=通用层（跨站型共享，pack_apply 永不触碰）
--    pack=站型层（随 pack 重建，引用保护）
-- 3) 通用六件套幂等入库（用户裁决 2026-09-23：首发/官种/禁转/国语/中字/DIY）
-- 4) 存量标签归类：现字典五条 → 合理的 group/layer 初值
-- 5) VIP（原「免费」位）标记 pack 层待运营处置（不删数据）

ALTER TABLE tag_dict
  ADD COLUMN IF NOT EXISTS tag_group TEXT NOT NULL DEFAULT 'attribute'
    CHECK (tag_group IN ('attribute', 'content'));
ALTER TABLE tag_dict
  ADD COLUMN IF NOT EXISTS scope_layer TEXT NOT NULL DEFAULT 'pack'
    CHECK (scope_layer IN ('global', 'pack'));

-- 存量归类：官方/官种/VIP 是属性；合集/带答案是内容。
-- 「官种」进通用层（六件套之一，同名 ON CONFLICT 也会归一）；
-- 其余站型层（education 口径），站型包接通后随 pack 重整。
UPDATE tag_dict SET tag_group = 'content' WHERE name IN ('合集', '带答案');

-- 通用六件套：幂等（同名改标 global 并归 attribute；不存在则插入，样式用 NP 原厂色）
INSERT INTO tag_dict (name, kind, scope, scope_layer, tag_group, bg_color, color, sort) VALUES
  ('首发', 'plain',   'torrent', 'global', 'attribute', '#8F77B5', '#ffffff', 60),
  ('官种', 'official','torrent', 'global', 'attribute', '#0000ee', '#ffffff', 55),
  ('禁转', 'plain',   'torrent', 'global', 'attribute', '#ff0000', '#ffffff', 50),
  ('国语', 'plain',   'torrent', 'global', 'attribute', '#6a3906', '#ffffff', 40),
  ('中字', 'plain',   'torrent', 'global', 'attribute', '#006400', '#ffffff', 35),
  ('DIY',  'plain',   'torrent', 'global', 'attribute', '#46d5ff', '#0c3a4a', 30)
ON CONFLICT (name) DO UPDATE
  SET scope_layer = 'global',
      tag_group   = 'attribute',
      scope       = 'torrent';
