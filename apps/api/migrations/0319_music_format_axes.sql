-- 0319_music_format_axes.sql
-- 站型成熟度 · 阶段 2 music 批（对标 §7.2「格式三元组」P0）：
--
-- What.CD/Gazelle 口径的检索三轴是 Media / Format / Encoding：
--   Media     = 载体（CD / WEB / Vinyl / SACD / DVD / Blu-ray）
--   Format    = 容器（FLAC / APE / WAV / ALAC / MP3 / AAC / OGG）
--   Encoding  = 音质档（Lossless / 24bit Lossless / 320 / V0 / Q8）
-- 我们现状把「无损 FLAC」当 media 词表项塞进 music 包（一个维度两种语义），
-- lossless 包 media 里混着「专辑/单曲/EP/现场」（发行类型语义）。
--
-- 本迁移把 music/lossless 两包的维度拆成正交三轴 + 发行类型：
--   media（载体）/ format（容器）/ encoding（音质档）/ releasetype（专辑形态）
-- 「standard（音质）」「audio_codec」两个旧维度从两包移除（站方在用的
-- 自定义维度不受影响——apply 只按包声明重建）。
--
-- 幂等：kinds 先剔除同 kind 再追加；dict 整段替换（固定值重放等值）。

BEGIN;

-- ============================================================
-- ① 维度定义：music / lossless 各自的 kinds 重排
-- ============================================================
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{kinds}',
  (SELECT jsonb_agg(e) FROM jsonb_array_elements(COALESCE(sections->'kinds','[]'::jsonb)) e
   WHERE e->>'kind' NOT IN ('media','standard','audio_codec','format','encoding','releasetype'))
  || '[
  {"kind":"media","label":"载体","sort":10},
  {"kind":"format","label":"格式","sort":20,"field_type":"select"},
  {"kind":"encoding","label":"音质档","sort":30,"field_type":"select"},
  {"kind":"releasetype","label":"发行类型","sort":40,"field_type":"select"}
]'::jsonb, true)
 WHERE code IN ('music','lossless');

-- ============================================================
-- ② 词表：正交三轴 + 发行类型（Gazelle 口径）
-- ============================================================
-- 载体（Media）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{dict,media}',
  '["CD","WEB","Vinyl","SACD","DVD","Blu-ray","Cassette"]'::jsonb, true)
WHERE code IN ('music','lossless');

-- 格式（Format = 容器）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{dict,format}',
  '["FLAC","APE","WAV","ALAC","MP3","AAC","OGG","DSD"]'::jsonb, true)
WHERE code IN ('music','lossless');

-- 音质档（Encoding）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{dict,encoding}',
  '["Lossless","24bit Lossless","320","V0 (VBR)","V2 (VBR)","Q8"]'::jsonb, true)
WHERE code IN ('music','lossless');

-- 发行类型（Release type = 专辑形态，Gazelle 九型收敛为常用六型）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{dict,releasetype}',
  '["Album","Single","EP","Live","Compilation","Soundtrack"]'::jsonb, true)
WHERE code IN ('music','lossless');

-- ============================================================
-- ③ 清掉两包里不再使用的旧维度词表（standard / audio_codec）
--    只动包载荷，不动 section_dict 表（apply 时按包重建）。
--    audio_codec 在 lossless 包里嵌在 sections.dict 下，要按嵌套键删。
-- ============================================================
UPDATE site_type_packs
SET sections = (sections - 'standard' - 'audio_codec')
               || jsonb_build_object('dict', (sections->'dict') - 'standard' - 'audio_codec')
WHERE code IN ('music','lossless');

COMMIT;
