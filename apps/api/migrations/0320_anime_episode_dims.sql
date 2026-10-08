-- 0320_anime_episode_dims.sql
-- 站型成熟度 · 阶段 2 anime 批（对标 §7.3「series↔话数」P0 的可落地半场）：
--
-- 动漫站的核心检索是「哪部番的第几话、哪个字幕组」。对标 AnimeBytes/U2：
--   series（番剧）→ 话数（episode）→ 字幕组版本（team）三层。
-- 我们已有的承载面：
--   · series 层：torrent_groups（0069，同名聚组 + 详情 GroupVersions）
--     + group_suggest（同名/trgm 启发式，发种时自动荐组）——**已在**；
--   · 字幕组层：subtitle_group/team 维度（0315/0317 词表已补）——**已在**；
--   · 话数层：**没有**——「第 1-12 话」只能塞标题字符串，检索与并列
--     展示全靠猜。
--
-- 本迁移补话数层（用六类型字段系统，零业务代码）：
--   · season   select  季（S1/S2/…/剧场版/OVA/SP——番剧语境的季是
--             发行批次而不是电视季，词表按 AB 口径）
--   · ep_first number  起始话（单话包 ep_first = ep_last）
--   · ep_last  number  结束话（全集包可空 = 未拆话）
-- number 维度走 sec_ep_first_min/max 区间筛选——「补第 5-8 话」检索
-- 直接可用；同番同话多字幕组并列 = group + ep 区间 + subtitle_group 三条件。
--
-- 同时把 0315 补的四维度（studio/subtitle_group/mal_id/anidb_id）的
-- 字段类型带齐进 anime 包 kinds（0315 时 apply 路径还是旧三列版本，
-- 包载荷没写 field_type——现路径已修，这里补上）。
--
-- 幂等：section_kinds ON CONFLICT DO NOTHING（field_type 不可改纪律）；
-- anime 包 kinds 先剔除同 kind 再追加；词表 NOT EXISTS 追加。

BEGIN;

-- ============================================================
-- ① 维度定义
-- ============================================================
INSERT INTO section_kinds
    (kind, label, sort, field_type, required, multiple, enabled)
VALUES
  ('season',   '季',      60, 'select', FALSE, FALSE, TRUE),
  ('ep_first', '起始话',  70, 'number', FALSE, FALSE, TRUE),
  ('ep_last',  '结束话',  80, 'number', FALSE, FALSE, TRUE)
ON CONFLICT (kind) DO NOTHING;

-- ============================================================
-- ② season 词表（AnimeBytes 口径：电视季按部数，非美剧 Fall/Spring）
-- ============================================================
INSERT INTO section_dict (kind, name, sort)
SELECT 'season', v.n, v.s FROM (VALUES
  ('第一季', 10), ('第二季', 20), ('第三季', 30),
  ('剧场版', 90), ('OVA/OAD', 100), ('SP/特别篇', 110)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='season' AND d.name=v.n);

-- ============================================================
-- ②b 存量维度安全升级 field_type
-- 「field_type 不可改」纪律防的是**有数据在解释**的维度；0315 批落库的
-- anime 四维度全库零引用（上方 INSERT 前提同样成立），且 text 落库的
-- 根因是当时 apply 路径丢字段（0313 已修，但 ON CONFLICT DO NOTHING
-- 不会回头改）——空引用时按本批声明补正，否则 select 词表永远挂不上。
-- ============================================================
UPDATE section_kinds SET field_type='select'
WHERE kind='season'
  AND field_type='text'
  AND NOT EXISTS (SELECT 1 FROM torrent_sections ts WHERE ts.kind='season');
UPDATE section_kinds SET field_type='multiselect'
WHERE kind='subtitle_group'
  AND field_type='text'
  AND NOT EXISTS (SELECT 1 FROM torrent_sections ts
                  WHERE ts.kind='subtitle_group');

-- ============================================================
-- ③ anime 包带上话数维度 + 0315 四维度的 field_type 补齐
-- ============================================================
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{kinds}',
  (SELECT jsonb_agg(e) FROM jsonb_array_elements(COALESCE(sections->'kinds','[]'::jsonb)) e
   WHERE e->>'kind' NOT IN ('season','ep_first','ep_last','studio',
                            'subtitle_group','mal_id','anidb_id'))
  || '[
  {"kind":"studio","label":"制作公司","sort":20,"field_type":"text"},
  {"kind":"subtitle_group","label":"字幕组","sort":30,"field_type":"multiselect"},
  {"kind":"mal_id","label":"MAL ID","sort":40,"field_type":"text"},
  {"kind":"anidb_id","label":"AniDB ID","sort":50,"field_type":"text"},
  {"kind":"season","label":"季","sort":60,"field_type":"select"},
  {"kind":"ep_first","label":"起始话","sort":70,"field_type":"number"},
  {"kind":"ep_last","label":"结束话","sort":80,"field_type":"number"}
]'::jsonb, true)
 WHERE code='anime';

-- anime 包 dict 补 season + subtitle_group 词表（字幕组词表此前只在
-- team 维度上——0315 声明的 subtitle_group kind 一直没配词表，等于装饰）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{dict,season}',
  '["第一季","第二季","第三季","剧场版","OVA/OAD","SP/特别篇"]'::jsonb, true)
WHERE code='anime';
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{dict,subtitle_group}',
  '["Sakura","LoliHouse","北宇治字幕组","幻樱字幕组","DHR動研社","千夏字幕组","ANi","无字幕"]'::jsonb, true)
WHERE code='anime';

-- 存量物化行同样补词表（NOT EXISTS 追加，幂等）
INSERT INTO section_dict (kind, name, sort)
SELECT 'subtitle_group', v.n, v.s FROM (VALUES
  ('Sakura',10),('LoliHouse',20),('北宇治字幕组',30),('幻樱字幕组',40),
  ('DHR動研社',50),('千夏字幕组',60),('ANi',70),('无字幕',80)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='subtitle_group' AND d.name=v.n);

COMMIT;
