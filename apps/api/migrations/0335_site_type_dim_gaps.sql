-- 0335_site_type_dim_gaps.sql
-- 站型成熟度补齐 · 批次 A（低风险加维度，纯数据零业务代码）。
-- 来源：_doc/站型成熟度对标与达标方案-总纲v2-2026-10-09.md §5 的 G4/G6/G12。
--
-- 补的维度（都是各内容域成熟站的招牌检索面）：
--   lossless/music  ← RED/OPS 口径：采样率、位深、Hi-Res 标识
--                     （LogScore/HasCue/三轴已在 0312/0319 落地，这里补硬件规格轴）
--   ebook           ← MAM 口径：扫描版 DPI、OCR 文字层
--   game            ← GGn 口径：DLC 状态、平台位数
--
-- 全部幂等：
--   ① section_kinds 用 ON CONFLICT (kind) DO NOTHING（不覆盖站长已改的 label/sort）；
--   ② section_dict 用 WHERE NOT EXISTS（该表只有主键 id，无 (kind,name) 唯一约束）；
--   ③ 站型包回填先剔除同名 kind 再追加（重跑不重复）。
-- ⚠️ kind 名必须小写 [a-z0-9_]（is_ascii_kind 守卫，见 pack_types.rs:15）。

BEGIN;

-- ============================================================
-- ① 维度定义
-- ============================================================
INSERT INTO section_kinds
    (kind, label, sort, field_type, required, multiple, enabled)
VALUES
  -- 无损/音乐：硬件规格三轴（RED/OPS 口径）
  ('sample_rate', '采样率',     120, 'select', FALSE, FALSE, TRUE),
  ('bitdepth',    '位深',       130, 'select', FALSE, FALSE, TRUE),
  ('hi_res',      'Hi-Res',     140, 'bool',   FALSE, FALSE, TRUE),
  -- 电子书：扫描版质量标记（MAM 口径）
  ('dpi',         '扫描分辨率', 120, 'select', FALSE, FALSE, TRUE),
  ('ocr',         'OCR文字层',  130, 'bool',   FALSE, FALSE, TRUE),
  -- 游戏：DLC 状态与平台位数（GGn 口径）
  ('dlc',         'DLC状态',    120, 'select', FALSE, FALSE, TRUE),
  ('arch',        '平台位数',   130, 'select', FALSE, FALSE, TRUE)
ON CONFLICT (kind) DO NOTHING;

-- ============================================================
-- ② 枚举维度字典项
-- ============================================================
-- 采样率（含 DSD 变体：DSD 的采样率是 MHz 级，单列表达比塞进位深更清楚）
INSERT INTO section_dict (kind, name, sort)
SELECT 'sample_rate', v.n, v.s FROM (VALUES
  ('44.1 kHz',10),('48 kHz',20),('88.2 kHz',30),('96 kHz',40),
  ('176.4 kHz',50),('192 kHz',60),('352.8 kHz',70),
  ('DSD64',80),('DSD128',90),('DSD256',100)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='sample_rate' AND d.name=v.n);

-- 位深
INSERT INTO section_dict (kind, name, sort)
SELECT 'bitdepth', v.n, v.s FROM (VALUES
  ('16 bit',10),('24 bit',20),('32 bit',30)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='bitdepth' AND d.name=v.n);

-- 扫描分辨率
INSERT INTO section_dict (kind, name, sort)
SELECT 'dpi', v.n, v.s FROM (VALUES
  ('300 dpi',10),('400 dpi',20),('600 dpi',30),
  ('1200 dpi',40),('其他',50)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='dpi' AND d.name=v.n);

-- DLC 状态（GGn：本体/含 DLC/仅 DLC 是发布语义的一部分）
INSERT INTO section_dict (kind, name, sort)
SELECT 'dlc', v.n, v.s FROM (VALUES
  ('无 DLC',10),('部分 DLC',20),('含全部 DLC',30),('仅 DLC',40)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='dlc' AND d.name=v.n);

-- 平台位数
INSERT INTO section_dict (kind, name, sort)
SELECT 'arch', v.n, v.s FROM (VALUES
  ('32 位',10),('64 位',20),('32+64 位',30)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='arch' AND d.name=v.n);

-- ============================================================
-- ③ 站型包回填：各站型只拿自己那几项
--    追加前剔除同名 kind ⇒ 幂等，重跑不重复。
-- ============================================================

-- lossless（无损站）：采样率 + 位深 + Hi-Res
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{kinds}',
  COALESCE((SELECT jsonb_agg(e) FROM jsonb_array_elements(COALESCE(sections->'kinds','[]'::jsonb)) e
            WHERE e->>'kind' NOT IN ('sample_rate','bitdepth','hi_res')),'[]'::jsonb)
  || '[
  {"kind":"sample_rate","label":"采样率","sort":120,"field_type":"select"},
  {"kind":"bitdepth","label":"位深","sort":130,"field_type":"select"},
  {"kind":"hi_res","label":"Hi-Res","sort":140,"field_type":"bool"}
]'::jsonb, true)
 WHERE code='lossless';

-- music（综合音乐站）：同样补硬件规格轴（音乐站也有无损资源）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{kinds}',
  COALESCE((SELECT jsonb_agg(e) FROM jsonb_array_elements(COALESCE(sections->'kinds','[]'::jsonb)) e
            WHERE e->>'kind' NOT IN ('sample_rate','bitdepth','hi_res')),'[]'::jsonb)
  || '[
  {"kind":"sample_rate","label":"采样率","sort":120,"field_type":"select"},
  {"kind":"bitdepth","label":"位深","sort":130,"field_type":"select"},
  {"kind":"hi_res","label":"Hi-Res","sort":140,"field_type":"bool"}
]'::jsonb, true)
 WHERE code='music';

-- ebook（电子书站）：扫描分辨率 + OCR 文字层
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{kinds}',
  COALESCE((SELECT jsonb_agg(e) FROM jsonb_array_elements(COALESCE(sections->'kinds','[]'::jsonb)) e
            WHERE e->>'kind' NOT IN ('dpi','ocr')),'[]'::jsonb)
  || '[
  {"kind":"dpi","label":"扫描分辨率","sort":120,"field_type":"select"},
  {"kind":"ocr","label":"OCR文字层","sort":130,"field_type":"bool"}
]'::jsonb, true)
 WHERE code='ebook';

-- game（游戏站）：DLC 状态 + 平台位数
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{kinds}',
  COALESCE((SELECT jsonb_agg(e) FROM jsonb_array_elements(COALESCE(sections->'kinds','[]'::jsonb)) e
            WHERE e->>'kind' NOT IN ('dlc','arch')),'[]'::jsonb)
  || '[
  {"kind":"dlc","label":"DLC状态","sort":120,"field_type":"select"},
  {"kind":"arch","label":"平台位数","sort":130,"field_type":"select"}
]'::jsonb, true)
 WHERE code='game';

COMMIT;
