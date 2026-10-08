-- 0321_movie_episode_dims.sql
-- 站型成熟度 · 阶段 2 movie 批（对标 §7.1「剧集话数结构化」P0 的可落地半场）。
--
-- 影视站的剧集检索与 anime 同构：哪部剧的第几季第几集、哪个版本
-- （WEB-DL/Remux/BluRay 由 source/codec 维度已承载）。0320 已给 anime
-- 落了 season/ep_first/ep_last，本迁移把同一组维度挂到 movie/documentary
-- 两包——影视语境的季是播出季（S1/S2/最终季），词表与 anime 的
-- 「部数季」分开配。
--
-- show→season→episode→版本 四层实体的承载评估（结论同步对标文档 §7.1）：
--   show 层   = torrent_groups（0069）+ GroupVersions 详情并列——已有；
--   版本层    = source/codec/standard 三轴（对标文档已标 ✅ 的族）——已有；
--   season/episode 层 = 本迁移的维度——补齐。
-- 即四层在「检索/并列」语义上闭合；不建新表（新表方案的增量价值在
-- 剧集日历/单集追更订阅，属 P1 深水区，另批评估）。
--
-- 幂等：kinds ON CONFLICT DO NOTHING（season 已存在，field_type 不动）；
-- 包 kinds 先剔除同 kind 再追加；词表 NOT EXISTS 追加。

BEGIN;

-- ============================================================
-- ① 维度定义（season 若不存在则按 select 建——新装库先跑本迁移时
--    0320 的 anime 维度尚未物化，section_dict 的 FK 指向 section_kinds）
-- ============================================================
INSERT INTO section_kinds
    (kind, label, sort, field_type, required, multiple, enabled)
VALUES
  ('season', '季', 60, 'select', FALSE, FALSE, TRUE),
  ('episode_first', '起始集', 75, 'number', FALSE, FALSE, TRUE),
  ('episode_last',  '结束集', 85, 'number', FALSE, FALSE, TRUE)
ON CONFLICT (kind) DO NOTHING;

-- ============================================================
-- ② 影视季词表（播出季口径；anime 0320 已配的部数季不冲突——同 kind
--    不同站点各自 apply 时重建词表，互不残留由 apply 的声明式重建保证）
-- ============================================================
INSERT INTO section_dict (kind, name, sort)
SELECT 'season', v.n, v.s FROM (VALUES
  ('第一季', 10), ('第二季', 20), ('第三季', 30), ('第四季', 40),
  ('最终季', 50), ('番外/迷你剧', 60)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='season' AND d.name=v.n);

-- ============================================================
-- ③ movie / documentary 包带上 season/episode 维度 + field_type 补齐
--    （movie 包现挂 team(空词表)；顺手把 0317 的字幕组语义接上）
-- ============================================================
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{kinds}',
  (SELECT jsonb_agg(e) FROM jsonb_array_elements(COALESCE(sections->'kinds','[]'::jsonb)) e
   WHERE e->>'kind' NOT IN ('season','episode_first','episode_last','subtitle_group'))
  || '[
  {"kind":"season","label":"季","sort":60,"field_type":"select"},
  {"kind":"episode_first","label":"起始集","sort":75,"field_type":"number"},
  {"kind":"episode_last","label":"结束集","sort":85,"field_type":"number"},
  {"kind":"subtitle_group","label":"字幕组","sort":95,"field_type":"multiselect"}
]'::jsonb, true)
 WHERE code IN ('movie','documentary');

-- 影视季词表进包 dict（apply 重建时带上）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{dict,season}',
  '["第一季","第二季","第三季","第四季","最终季","番外/迷你剧"]'::jsonb, true)
WHERE code IN ('movie','documentary');
-- 字幕组词表（影视语境：压制组）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{dict,subtitle_group}',
  '["CHDBits","OurBits","HDHome","MTeam","HDSky","无字幕"]'::jsonb, true)
WHERE code IN ('movie','documentary');

COMMIT;
