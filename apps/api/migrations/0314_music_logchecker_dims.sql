-- 0314_music_logchecker_dims.sql
-- 音乐站内容模型（对标 RED/Gazelle 音乐分支的成熟标准）。
--
-- 背景：站型包此前写section_kinds 只落 (kind,label,sort)，
-- field_type/required/multiple/enabled 四列丢失并回落列缺省
-- （select/非必填/单值），导致站型包无法表达自由值维度
-- （本次已随 pack_core.rs 修复：走 pack_kinds::kind_from_json 九列全字段）。
-- 本迁移把 music 站从「3 个单选枚举维度」升级为成熟音乐站内容模型：
--新增 artist/album/log_score/haslog/cue/scene/releasetype 七个维度。
--
-- 口径说明（成熟站对标，_doc/各站型成熟度对标与达标方案.md §2.3）：
--   log_score  number  ← Logchecker 日志分（100/99/…；暂由上传侧填，
--                        与后续 Logchecker 专项对接，本批先立字段与筛选面）
--   haslog     bool    ← Gazelle haslog：有 log 且100%（筛选主入口）
--   cue        bool    ← Gazelle hascue：内嵌 cue
--   scene      select  ← Gazelle scene：非签到直发
--   releasetype select ← Gazelle releasetype：专辑/单曲/EP/原声/混音
--   artist/album text  ← TGroup 聚合键（Gazelle artistname/groupname 口径）
--
-- 全部走 INSERT ... ON CONFLICT DO NOTHING：不覆盖站长已改过的
-- label/sort/field_type（field_type 不可改是既有纪律，见 pack_core / admin PUT）。
-- dict 项同理ON CONFLICT 去重。

-- 1) 新增维度（media/standard/source 已在 0092 配好，此处不重复）
INSERT INTO section_kinds
    (kind, label, sort, field_type, required, multiple, enabled)
VALUES
    ('artist',      '艺术家',   20, 'text',   FALSE, FALSE, TRUE),
    ('album',       '专辑',     30, 'text',   FALSE, FALSE, TRUE),
    ('releasetype', '发行类型', 40, 'select', FALSE, FALSE, TRUE),
    ('log_score',   '日志分',   80, 'number', FALSE, FALSE, TRUE),
    ('haslog',      '100%日志', 90, 'bool',   FALSE, FALSE, TRUE),
    ('cue',         '内嵌CUE', 100, 'bool',  FALSE, FALSE, TRUE),
    ('scene',       'SCENE',    110, 'select', FALSE, FALSE, TRUE)
ON CONFLICT (kind) DO NOTHING;

-- 2) 枚举维度的字典项（releasetype / scene）
-- section_dict 只有主键 id、没有 (kind,name) 唯一约束，故用 WHERE NOT EXISTS
-- 幂等插入（对齐 pack_core.rs 的既有写法），不能用 ON CONFLICT DO NOTHING。
INSERT INTO section_dict (kind, name, sort)
SELECT 'releasetype', v.n, v.s
  FROM (VALUES ('专辑',10),('单曲',20),('EP',30),('原声',40),('混音',50),
               ('Live',60),('影视原声',70)) AS v(n, s)
 WHERE NOT EXISTS (
   SELECT 1 FROM section_dict d
    WHERE d.kind = 'releasetype' AND d.name = v.n);

INSERT INTO section_dict (kind, name, sort)
SELECT 'scene', v.n, v.s
  FROM (VALUES ('直发',10),('Scene',20)) AS v(n, s)
 WHERE NOT EXISTS (
   SELECT 1 FROM section_dict d
    WHERE d.kind = 'scene' AND d.name = v.n);

-- 3) 站型包回填：把七个维度写进 music / lossless 两个站型的 sections.kinds，
--    使「导出站型包 → 换站重放」不丢这些维度（apply 路径本次已能保六类型）。
--    ⚠️ 幂等：先剔除这七个 kind 再追加。直接 `||` 追加会让重跑
--    （撞号让位后删记录重跑 / 手动灌库后 sqlx 再跑一次）**重复追加**，
--    同一 kind 在包里出现多份→ apply 时反复 INSERT 同一维度。
UPDATE site_type_packs
   SET sections = jsonb_set(
         COALESCE(sections, '{}'::jsonb),
         '{kinds}',
         COALESCE((
           SELECT jsonb_agg(e)
             FROM jsonb_array_elements(
                    COALESCE(sections->'kinds', '[]'::jsonb)) e
            WHERE e->>'kind'
                  NOT IN ('artist','album','releasetype','log_score',
                           'haslog','cue','scene')
         ), '[]'::jsonb)
           || '[
  {"kind":"artist","label":"艺术家","sort":20,"field_type":"text","required":false,"multiple":false,"enabled":true},
  {"kind":"album","label":"专辑","sort":30,"field_type":"text","required":false,"multiple":false,"enabled":true},
  {"kind":"releasetype","label":"发行类型","sort":40,"field_type":"select","required":false,"multiple":false,"enabled":true},
  {"kind":"log_score","label":"日志分","sort":80,"field_type":"number","required":false,"multiple":false,"enabled":true},
  {"kind":"haslog","label":"100%日志","sort":90,"field_type":"bool","required":false,"multiple":false,"enabled":true},
  {"kind":"cue","label":"内嵌CUE","sort":100,"field_type":"bool","required":false,"multiple":false,"enabled":true},
  {"kind":"scene","label":"SCENE","sort":110,"field_type":"select","required":false,"multiple":false,"enabled":true}
]'::jsonb,
         true)
 WHERE code IN ('music', 'lossless');

COMMIT;