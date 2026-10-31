-- 0442: section_dict 两代词表收口——codec/media 去重归一（B 修复：候选乱）。
--
-- 根因：0422 站型模式批给 codec/media 种第二代词表时用 WHERE NOT EXISTS
-- 按名查重，而第一代（0175 通用缺省批）名字不同（H.264 vs H.264/x264、
-- XviD vs Xvid；media 第一代是「内容形态」视频/音频/文档…，第二代才是
-- 「媒介」原盘/Remux/WEB-DL…），两代在库里共存 ⇒ 发布页下拉重复且语义错位。
-- 引用体检（2026-10-31 服务器实测）：gen1 codec 19 条引用（全是 H.265）；
-- gen1 media（内容形态组）零引用；gen2 media 23 条、gen2 codec 4 条。
--
-- 修法（三步，全部幂等）：
--   ① 引用迁移：gen1 codec 的种子引用按名字映射到 gen2（H.264→H.264/x264、
--      H.265→H.265/x265、XviD→Xvid；AV1 两代同名无需迁）；
--   ② 删 gen1 行：codec 第一代 8 行（FLAC/MP3/APE 是音频编码，本就不该在
--      视频编码组，直接删无引用）+ media 第一代 8 行（内容形态，零引用，
--      语义归「资源类型/分类」不归媒介）；
--   ③ 断根：0175 的通用缺省种子已不会重放（迁移只跑一次），无需改老迁移
--      （checksum 锁死）；若未来再有「按名补种」批次，先跑本收口。

-- ① 种子引用 gen1 → gen2（按名字映射；已指向 gen2 的不动）
UPDATE torrent_sections ts
   SET dict_id = gen2.id
 FROM section_dict gen1
 JOIN section_dict gen2
   ON gen2.kind = gen1.kind
  AND gen2.name = CASE gen1.name
       WHEN 'H.264' THEN 'H.264/x264'
       WHEN 'H.265' THEN 'H.265/x265'
       WHEN 'XviD'  THEN 'Xvid'
       ELSE gen1.name END
WHERE ts.kind = 'codec'
  AND ts.dict_id = gen1.id
  AND gen1.id BETWEEN 422 AND 429
  AND gen1.name <> gen2.name;

-- ②a 删 codec 第一代（含错位的音频编码 FLAC/MP3/APE；'其他' 429 与 gen2
-- 语义相同但无引用，直接删旧留新）
DELETE FROM torrent_sections
 WHERE kind = 'codec' AND dict_id IN (SELECT id FROM section_dict
   WHERE kind = 'codec' AND name IN
   ('H.264','H.265','XviD','FLAC','MP3','APE','其他') AND id BETWEEN 422 AND 429);
DELETE FROM section_dict
 WHERE kind = 'codec' AND name IN
   ('H.264','H.265','XviD','FLAC','MP3','APE')
   AND id BETWEEN 422 AND 429;
DELETE FROM section_dict WHERE id = 429 AND kind = 'codec' AND name = '其他';

-- ②b 删 media 第一代（内容形态组，零引用；'其他' 437 同上删旧）
DELETE FROM torrent_sections
 WHERE kind = 'media' AND dict_id IN (SELECT id FROM section_dict
   WHERE kind = 'media' AND name IN
   ('视频','音频','文档','图片','软件','书籍','游戏','其他') AND id BETWEEN 430 AND 437);
DELETE FROM section_dict
 WHERE kind = 'media' AND id BETWEEN 430 AND 437;
