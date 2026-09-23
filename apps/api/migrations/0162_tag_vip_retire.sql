-- 0162_tag_vip_retire.sql — VIP 标签退役 + 站型词表校订（0161 修订版）
--
-- 1) VIP（原「免费」位，id=2）退役：用户裁决 2026-09-23。
--    该标签与促销系统职责重叠（免费=促销字段），且 seed 命名历经 免费→VIP 漂移，
--    留在词表只制造歧义。存量引用先摘除再删字典行（FK CASCADE 兜底）。
-- 2) 站型专属词表校订（对标策划案裁决 5 + NP 各站实践）：
--    - movie 补「禁2压」类硬防护位缺口 → 用「内嵌中字」（属性，WEB-DL 常用标注）
--      +「预告」（内容）——按 PT 实际高频标签收敛，不追新造词
--    - anime 补「合集」已有；补「地区限定」类不引入——保持 3-4 词的小口径
--    - game 的「DLC完整」→「DLC 完整」（空格对齐中文排版）
--    - education/music/lossless/ebook/documentary 维持（口径已对）
--    - 全部站型补「完结」到 attribute（剧集/漫画/连载类通用收尾标注，
--      原 movie 独占不合理——anime/game 的连载内容同样需要）

-- 1) VIP 退役（幂等：不存在时零行）
DELETE FROM tags WHERE tag_id IN (SELECT id FROM tag_dict WHERE name = 'VIP');
DELETE FROM tag_dict WHERE name = 'VIP' AND scope = 'torrent';

-- 2) 站型词表校订
UPDATE site_type_packs SET tags = '[
  {"name": "完结", "group": "attribute"},
  {"name": "内嵌中字", "group": "attribute"},
  {"name": "导演剪辑", "group": "content"},
  {"name": "合集", "group": "content"},
  {"name": "预告", "group": "content"}
]'::jsonb WHERE code = 'movie';

UPDATE site_type_packs SET tags = '[
  {"name": "完结", "group": "attribute"},
  {"name": "官译", "group": "attribute"},
  {"name": "剧场版", "group": "content"},
  {"name": "合集", "group": "content"}
]'::jsonb WHERE code = 'anime';

UPDATE site_type_packs SET tags = '[
  {"name": "完结", "group": "attribute"},
  {"name": "官方汉化", "group": "attribute"},
  {"name": "DLC 完整", "group": "content"},
  {"name": "合集", "group": "content"}
]'::jsonb WHERE code = 'game';

-- education/music/lossless/ebook/documentary：补「完结」（剧集/专辑连载口径）
UPDATE site_type_packs SET tags = tags || '[
  {"name": "完结", "group": "attribute"}
]'::jsonb
 WHERE code IN ('education', 'music', 'lossless', 'ebook', 'documentary')
   AND NOT tags::text LIKE '%完结%';
