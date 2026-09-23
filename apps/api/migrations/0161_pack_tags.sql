-- 0161_pack_tags.sql — 站型包 tags 数据节种子（0160 P2 配套）
-- 仅站型专属层（apply 只重建 scope_layer='pack'；通用六件套由 0160 基线维护）。
-- 口径来源：策划案裁决 5 的站型标签集修订版 + NP 原厂/各站实践。

ALTER TABLE site_type_packs
  ADD COLUMN IF NOT EXISTS tags JSONB NOT NULL DEFAULT '[]'::jsonb;

-- education（好学口径：官方出品/合集/带答案，原五件套去掉 VIP 与通用层重复的官种）
UPDATE site_type_packs SET tags = '[
  {"name": "官方", "kind": "official", "group": "attribute"},
  {"name": "合集", "group": "content"},
  {"name": "带答案", "group": "content"}
]'::jsonb WHERE code = 'education';

-- movie
UPDATE site_type_packs SET tags = '[
  {"name": "完结", "group": "attribute"},
  {"name": "导演剪辑", "group": "content"},
  {"name": "合集", "group": "content"}
]'::jsonb WHERE code = 'movie';

-- anime
UPDATE site_type_packs SET tags = '[
  {"name": "官译", "group": "attribute"},
  {"name": "剧场版", "group": "content"},
  {"name": "合集", "group": "content"}
]'::jsonb WHERE code = 'anime';

-- game
UPDATE site_type_packs SET tags = '[
  {"name": "官方汉化", "group": "attribute"},
  {"name": "DLC完整", "group": "content"},
  {"name": "合集", "group": "content"}
]'::jsonb WHERE code = 'game';

-- music / lossless（音乐站：官方发行/现场/合集）
UPDATE site_type_packs SET tags = '[
  {"name": "官方", "kind": "official", "group": "attribute"},
  {"name": "现场", "group": "content"},
  {"name": "合集", "group": "content"}
]'::jsonb WHERE code IN ('music', 'lossless');

-- ebook / documentary（内容向：官方/合集足矣，通用层已覆盖大半）
UPDATE site_type_packs SET tags = '[
  {"name": "官方", "kind": "official", "group": "attribute"},
  {"name": "合集", "group": "content"}
]'::jsonb WHERE code IN ('ebook', 'documentary');

-- general / sports / software：只靠通用六件套，无站型专属词
UPDATE site_type_packs SET tags = '[]'::jsonb
 WHERE code IN ('general', 'sports', 'software');
