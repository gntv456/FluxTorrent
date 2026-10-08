-- 0315_site_type_content_models.sql
-- 站型内容模型批量配置（第二批）：ebook / game / anime / software /
-- sports / education 六站型的专属维度（对标各域成熟站）。
--
-- 承接 0314（music/lossless）。前提：站型包 apply 路径已修
-- （staff_http/pack_types.rs::apply_pack_kinds 走 pack_kinds::kind_from_json
-- 九列全字段），故此处配的 field_type 能真正落库，不再静默降级回 select。
--
-- 竞品对标（详见 _doc/各站型成熟度对标与达标方案.md §3b）：
--   ebook     ← MAM：author/narrator/DRM/format/series 结构化书目
--   game      ← GGn：platform/DRM-Free/region、GameDOX 聚合
--   anime     ← AnimeBytes/U2：studio/字幕组、MAL/AniDB 关联
--   software  ← 通用软件 PT：os/version/license
--   sports    ← BTN season 模型：league/season/round
--   education ← 包子PT：subject/grade/resource_type
--
-- 全部幂等：
--   ① section_kinds 用 ON CONFLICT DO NOTHING（不覆盖站长已改的label/sort）；
--      field_type 不可改是既有纪律，故不 UPDATE 类型列。
--   ② section_dict 用 WHERE NOT EXISTS（该表只有主键 id，无 (kind,name) 唯一约束）。
--   ③ 站型包回填先剔除同名 kind 再追加（重跑不重复）。
-- ⚠️ 维度 kind 名用小写 [a-z0-9_]（section_kinds 的既有约束）。

-- ============================================================
-- ① 维度定义（六类型：text/number/select/multiselect/date/bool）
-- ============================================================
INSERT INTO section_kinds
    (kind, label, sort, field_type, required, multiple, enabled)
VALUES
  -- 电子书（MAM 口径：结构化书目 + DRM 显式 + 有声书旁白）
  ('author',      '作者',       20, 'text',   FALSE, FALSE, TRUE),
  ('narrator',    '旁白',       30, 'text',   FALSE, FALSE, TRUE),
  ('isbn',        'ISBN',      40, 'text',   FALSE, FALSE, TRUE),
  ('series',      '丛书',       50, 'text',   FALSE, FALSE, TRUE),
  ('bookformat',  '电子书格式', 60, 'select', FALSE, FALSE, TRUE),
  ('drm',         'DRM',       70, 'bool',   FALSE, FALSE, TRUE),
  -- 游戏（GGn 口径：平台 + DRM-Free 显式标签）
  ('platform',    '平台',      20, 'select', FALSE, FALSE, TRUE),
  ('gametype',    '游戏类型',   30, 'select', FALSE, FALSE, TRUE),
  ('region',      '发行区',     40, 'select', FALSE, FALSE, TRUE),
  ('drm_free',    ' DRM免费',  50, 'bool',   FALSE, FALSE, TRUE),
  -- 动漫（AnimeBytes/U2 口径：制作方 + 字幕组 + MAL/AniDB 关联）
  ('studio',      '制作公司',   20, 'text',   FALSE, FALSE, TRUE),
  ('subtitle_group', '字幕组', 30, 'text',   FALSE, FALSE, TRUE),
  ('mal_id',      'MAL ID',   40, 'text',   FALSE, FALSE, TRUE),
  ('anidb_id',    'AniDB ID', 50, 'text',   FALSE, FALSE, TRUE),
  -- 软件（os / version / license）
  ('os',          '操作系统',   20, 'select', FALSE, FALSE, TRUE),
  ('version',     '版本',      30, 'text',   FALSE, FALSE, TRUE),
  ('license_type','授权类型',   40, 'select', FALSE, FALSE, TRUE),
  -- 体育（BTN season 模型：联赛/赛季/轮次）
  ('league',      '联赛',      20, 'select', FALSE, FALSE, TRUE),
  ('season',      '赛季',      30, 'text',   FALSE, FALSE, TRUE),
  ('round',       '轮次',      40, 'text',   FALSE, FALSE, TRUE),
  -- 教育（包子PT 口径：学科/学段/资源类型）
  ('subject',     '学科',      20, 'select', FALSE, FALSE, TRUE),
  ('grade',       '适用年级',   30, 'select', FALSE, FALSE, TRUE),
  ('resource_type','资源类型', 40, 'select', FALSE, FALSE, TRUE)
ON CONFLICT (kind) DO NOTHING;

-- ============================================================
-- ② 枚举维度字典项
-- ============================================================
-- 电子书格式（MAM 口径）
INSERT INTO section_dict (kind, name, sort)
SELECT 'bookformat', v.n, v.s FROM (VALUES
  ('EPUB',10),('PDF',20),('MOBI',30),('AZW3',40),
  ('CBZ',50),('CBR',60),('DjVu',70),('TXT',80)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='bookformat' AND d.name=v.n);

-- 游戏平台（GGn 口径：NES→Switch + PC/Mac/移动）
INSERT INTO section_dict (kind, name, sort)
SELECT 'platform', v.n, v.s FROM (VALUES
  ('PC',10),('Mac',20),('Linux',30),
  ('PlayStation',40),('Xbox',50),('Nintendo Switch',60),
  ('掌机',70),('移动端',80)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='platform' AND d.name=v.n);

INSERT INTO section_dict (kind, name, sort)
SELECT 'gametype', v.n, v.s FROM (VALUES
  ('单机',10),('联机',20),('多人',30),
  ('游戏本体',40),('游戏原声/OST',50),('GameDOX',60),
  ('更新包',70),('MOD',80), ('攻略',90)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='gametype' AND d.name=v.n);

INSERT INTO section_dict (kind, name, sort)
SELECT 'region', v.n, v.s FROM (VALUES
  ('全球',10),('欧美',20),('日韩',30),('中国',40),('其他',50)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='region' AND d.name=v.n);

-- 授权类型
INSERT INTO section_dict (kind, name, sort)
SELECT 'license_type', v.n, v.s FROM (VALUES
  ('免费/Free',10),('开源',20),('试用',30),
  ('商业授权',40),('教育授权',50)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='license_type' AND d.name=v.n);

-- 操作系统
INSERT INTO section_dict (kind, name, sort)
SELECT 'os', v.n, v.s FROM (VALUES
  ('Windows',10),('macOS',20),('Linux',30),
  ('Android',40),('iOS',50),('跨平台',60)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='os' AND d.name=v.n);

-- 学科 / 学段 / 资源类型（教育站）
INSERT INTO section_dict (kind, name, sort)
SELECT 'subject', v.n, v.s FROM (VALUES
  ('语文',10),('数学',20),('英语',30),('物理',40),
  ('化学',50),('生物',60),('政治',70),('历史',80),
  ('地理',90),('综合',100)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='subject' AND d.name=v.n);

INSERT INTO section_dict (kind, name, sort)
SELECT 'grade', v.n, v.s FROM (VALUES
  ('学前',10),('小学',20),('初中',30),('高中',40),
  ('大学',50),('研究生',60),('成人',70)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='grade' AND d.name=v.n);

INSERT INTO section_dict (kind, name, sort)
SELECT 'resource_type', v.n, v.s FROM (VALUES
  ('教材',10),('课件/PPT',20),('试卷',30),('真题',40),
  ('网课',50),('大纲',60),('教辅',70),('其他',80)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='resource_type' AND d.name=v.n);

-- ============================================================
-- ③ 站型包回填：各站型带上**自己**的专属维度
--    走 apply 路径导出→重放不丢（apply 已修，可保 field_type）。
--    · 只追加，不动各站型既有维度（media/source/team/grades/...）。
--    · 每站型只拿自己那几项（不是全量塞给所有站型）。
--    · 追加前剔除同名 kind ⇒ 幂等，重跑不重复。
-- ============================================================

-- 各自维度数组（与 ① 的 field_type 一致；label/sort 同 ①）
-- ebook（MAM 口径）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{kinds}',
  COALESCE((SELECT jsonb_agg(e) FROM jsonb_array_elements(COALESCE(sections->'kinds','[]'::jsonb)) e
            WHERE e->>'kind' NOT IN ('author','narrator','isbn','series','bookformat','drm')),'[]'::jsonb)
  || '[
  {"kind":"author","label":"作者","sort":20,"field_type":"text"},
  {"kind":"narrator","label":"旁白","sort":30,"field_type":"text"},
  {"kind":"isbn","label":"ISBN","sort":40,"field_type":"text"},
  {"kind":"series","label":"丛书","sort":50,"field_type":"text"},
  {"kind":"bookformat","label":"电子书格式","sort":60,"field_type":"select"},
  {"kind":"drm","label":"DRM","sort":70,"field_type":"bool"}
]'::jsonb, true)
 WHERE code='ebook';

-- game（GGn 口径：平台/DRM-Free 显式标签）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{kinds}',
  COALESCE((SELECT jsonb_agg(e) FROM jsonb_array_elements(COALESCE(sections->'kinds','[]'::jsonb)) e
            WHERE e->>'kind' NOT IN ('platform','gametype','region','drm_free')),'[]'::jsonb)
  || '[
  {"kind":"platform","label":"平台","sort":20,"field_type":"select"},
  {"kind":"gametype","label":"游戏类型","sort":30,"field_type":"select"},
  {"kind":"region","label":"发行区","sort":40,"field_type":"select"},
  {"kind":"drm_free","label":"DRM免费","sort":50,"field_type":"bool"}
]'::jsonb, true)
 WHERE code='game';

-- anime（AnimeBytes/U2 口径：制作方 + 字幕组 + MAL/AniDB）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{kinds}',
  COALESCE((SELECT jsonb_agg(e) FROM jsonb_array_elements(COALESCE(sections->'kinds','[]'::jsonb)) e
            WHERE e->>'kind' NOT IN ('studio','subtitle_group','mal_id','anidb_id')),'[]'::jsonb)
  || '[
  {"kind":"studio","label":"制作公司","sort":20,"field_type":"text"},
  {"kind":"subtitle_group","label":"字幕组","sort":30,"field_type":"text"},
  {"kind":"mal_id","label":"MAL ID","sort":40,"field_type":"text"},
  {"kind":"anidb_id","label":"AniDB ID","sort":50,"field_type":"text"}
]'::jsonb, true)
 WHERE code='anime';

-- software
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{kinds}',
  COALESCE((SELECT jsonb_agg(e) FROM jsonb_array_elements(COALESCE(sections->'kinds','[]'::jsonb)) e
            WHERE e->>'kind' NOT IN ('os','version','license_type')),'[]'::jsonb)
  || '[
  {"kind":"os","label":"操作系统","sort":20,"field_type":"select"},
  {"kind":"version","label":"版本","sort":30,"field_type":"text"},
  {"kind":"license_type","label":"授权类型","sort":40,"field_type":"select"}
]'::jsonb, true)
 WHERE code='software';

-- sports（BTN season 模型：联赛/赛季/轮次）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{kinds}',
  COALESCE((SELECT jsonb_agg(e) FROM jsonb_array_elements(COALESCE(sections->'kinds','[]'::jsonb)) e
            WHERE e->>'kind' NOT IN ('league','season','round')),'[]'::jsonb)
  || '[
  {"kind":"league","label":"联赛","sort":20,"field_type":"select"},
  {"kind":"season","label":"赛季","sort":30,"field_type":"text"},
  {"kind":"round","label":"轮次","sort":40,"field_type":"text"}
]'::jsonb, true)
 WHERE code='sports';

-- education（包子PT 口径：学科/学段/资源类型）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{kinds}',
  COALESCE((SELECT jsonb_agg(e) FROM jsonb_array_elements(COALESCE(sections->'kinds','[]'::jsonb)) e
            WHERE e->>'kind' NOT IN ('subject','grade','resource_type')),'[]'::jsonb)
  || '[
  {"kind":"subject","label":"学科","sort":20,"field_type":"select"},
  {"kind":"grade","label":"适用年级","sort":30,"field_type":"select"},
  {"kind":"resource_type","label":"资源类型","sort":40,"field_type":"select"}
]'::jsonb, true)
 WHERE code='education';

COMMIT;