-- 0175: 跨站型通用的初始维度（媒介 / 编码 / 来源 / 制作组）
--
-- 定位：本项目是通用建站，维度归站长自定义；0085 预置的九维里「学段/版本/音频
-- 编码/规格/处理工艺」是教育站与影视站的专属口径，不该作为所有站型的初始状态。
-- 本迁移只做**加法**：补齐四维通用维度与其默认选项，并把 general 站型包的
-- sections 预置补上（该列此前为 NULL，导致应用它时按 0101 的清理逻辑把维度删空）。
-- 不删任何已有维度——已被种子引用的维度删了会连带清掉归属，未引用的让站长自己删。

-- 1) 四维通用维度（sort 沿用 0085 里同名维度的位次，避免与既有排序打架）
INSERT INTO section_kinds (kind, label, sort) VALUES
  ('media',  '媒介',   10),
  ('codec',  '编码',   40),
  ('source', '来源',   70),
  ('team',   '制作组', 90)
ON CONFLICT (kind) DO NOTHING;

-- 2) 默认选项：仅在该维度当前一个选项都没有时铺底，绝不覆盖站长改过的词表
INSERT INTO section_dict (kind, name, sort)
SELECT 'media', v.name, v.i FROM (VALUES
  (1, '视频'), (2, '音频'), (3, '文档'), (4, '图片'),
  (5, '软件'), (6, '书籍'), (7, '游戏'), (8, '其他')
) AS v(i, name)
WHERE NOT EXISTS (SELECT 1 FROM section_dict sd WHERE sd.kind = 'media');

INSERT INTO section_dict (kind, name, sort)
SELECT 'codec', v.name, v.i FROM (VALUES
  (1, 'H.264'), (2, 'H.265'), (3, 'AV1'), (4, 'XviD'),
  (5, 'FLAC'), (6, 'MP3'), (7, 'APE'), (8, '其他')
) AS v(i, name)
WHERE NOT EXISTS (SELECT 1 FROM section_dict sd WHERE sd.kind = 'codec');

INSERT INTO section_dict (kind, name, sort)
SELECT 'source', v.name, v.i FROM (VALUES
  (1, 'WEB-DL'), (2, 'WEBRip'), (3, 'BluRay'), (4, 'HDTV'),
  (5, 'DVDRip'), (6, 'CD'), (7, 'SACD'), (8, '自制'), (9, '其他')
) AS v(i, name)
WHERE NOT EXISTS (SELECT 1 FROM section_dict sd WHERE sd.kind = 'source');
-- team（制作组）刻意不预置选项：制作组名单每站都不同，由站长或发布流程自建

-- 3) general 包的 sections 预置（此前 NULL → 应用它等于"本包不管维度"，
--    修好 pack_apply 后不再删维度，但也要让包能主动把维度立起来）
UPDATE site_type_packs SET sections = $$
{
  "kinds": [
    { "kind": "media",  "label": "媒介",   "sort": 10 },
    { "kind": "codec",  "label": "编码",   "sort": 40 },
    { "kind": "source", "label": "来源",   "sort": 70 },
    { "kind": "team",   "label": "制作组", "sort": 90 }
  ],
  "dict": {
    "media":  ["视频", "音频", "文档", "图片", "软件", "书籍", "游戏", "其他"],
    "codec":  ["H.264", "H.265", "AV1", "XviD", "FLAC", "MP3", "APE", "其他"],
    "source": ["WEB-DL", "WEBRip", "BluRay", "HDTV", "DVDRip", "CD", "SACD", "自制", "其他"],
    "team":   []
  }
}
$$::jsonb
WHERE code = 'general' AND sections IS NULL;
