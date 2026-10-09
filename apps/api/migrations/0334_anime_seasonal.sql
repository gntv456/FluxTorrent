-- 0334_anime_seasonal.sql
-- 站型成熟度 · 阶段 2 anime 批（续 0320 话数层，对标 §7.3）：
--
-- ① 播出季度 `air_season` —— 动漫站**门户级**维度。
--    0320 的 `season` 是「第几季」（第一季/第二季，作品内部批次）；
--    与之正交的是「哪一年哪一季播出」（2026夏）。蜜柑 / AnimeBytes /
--    U2 / 动漫花园的门面都是按播出季分栏的「当季新番」——我们没有承载，
--    只能把这个信息塞进标题字符串，检索与门户都做不出来。
--
-- ② `bangumi_id` —— 包 metadata.sources 早就声明了 `bangumi`
--    （ptgen.rs 已代理 bgm.tv / bangumi.tv），却只有 mal_id / anidb_id
--    两列承载，缺它。华语动漫站把 Bangumi 当第一外部 ID，补齐一致性。
--
-- 幂等：section_kinds ON CONFLICT DO NOTHING；anime 包 kinds 先剔同 kind
-- 再追加（COALESCE 兜空集——jsonb_agg 空集返 NULL 的坑见 CONTRIBUTING 1c）；
-- 词表 NOT EXISTS 追加。

BEGIN;

-- ============================================================
-- ① 维度定义
-- ============================================================
INSERT INTO section_kinds
    (kind, label, sort, field_type, required, multiple, enabled)
VALUES
  ('bangumi_id', 'Bangumi ID', 45, 'text',   FALSE, FALSE, TRUE),
  ('air_season', '播出季',     55, 'select', FALSE, FALSE, TRUE)
ON CONFLICT (kind) DO NOTHING;

-- ============================================================
-- ② air_season 词表（"YYYY季"格式：便于目视、可按名解析排季）
--    近两年打底，站长可在后台续加。sort 单调递增=时间序，
--    「当季」= sort 最大的那项（读口据此定当季）。
-- ============================================================
INSERT INTO section_dict (kind, name, sort)
SELECT 'air_season', v.n, v.s FROM (VALUES
  ('2025春', 10), ('2025夏', 20), ('2025秋', 30), ('2025冬', 40),
  ('2026春', 50), ('2026夏', 60), ('2026秋', 70), ('2026冬', 80)
) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind = 'air_season' AND d.name = v.n);

-- ============================================================
-- ③ anime 包挂上两维度（先剔同 kind 再追加，幂等）
-- ============================================================
UPDATE site_type_packs SET sections = jsonb_set(
  COALESCE(sections, '{}'::jsonb), '{kinds}',
  COALESCE((SELECT jsonb_agg(e)
              FROM jsonb_array_elements(COALESCE(sections->'kinds','[]'::jsonb)) e
             WHERE e->>'kind' NOT IN ('air_season','bangumi_id')), '[]'::jsonb)
  || '[
  {"kind":"bangumi_id","label":"Bangumi ID","sort":45,"field_type":"text"},
  {"kind":"air_season","label":"播出季","sort":55,"field_type":"select"}
]'::jsonb, true)
WHERE code = 'anime';

-- anime 包 dict 补 air_season 词表（供 apply 时重建/追加，与 section_dict 同源）
UPDATE site_type_packs SET sections = jsonb_set(
  COALESCE(sections, '{}'::jsonb), '{dict,air_season}',
  '["2025春","2025夏","2025秋","2025冬","2026春","2026夏","2026秋","2026冬"]'::jsonb,
  true)
WHERE code = 'anime';

COMMIT;
