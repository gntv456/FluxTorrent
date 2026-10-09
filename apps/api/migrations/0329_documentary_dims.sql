-- 0329_documentary_dims.sql
-- ⚠️ 号位说明：本迁移原建为 0328，与并行会话的 0328_subs_calendar.sql 撞号
-- （双方都在建号前未复查目录）。为让两批都能在**新装库**上执行，本批让号
-- 改为 0329；已部署库的记录由仓库纪律处理（见 .workbuddy/memory 记录）。
-- 站型成熟度 · 阶段 2 documentary 批（纪录片站型专项）。
--
-- 更正历史判断：旧方案把纪录片写成「movie 的轻配置档、几乎无专属缺口」
-- （见 _doc/各站型成熟度对标与达标方案.md §2.11 初版）。代码实证后**不成立**：
--
--   ① movie 包有 standard（规格/画质档：1080p/2160p/REMUX），documentary
--      **没有**——而「找 4K 自然纪录片」是纪录片站最高频检索之一；
--   ② documentary 把 BBC/NHK 塞进 team 维度（语义=制作组/发布组），
--      而 BBC/NHK 是**出品方/播出机构**（network/studio）概念，混用会让
--      「按出品方浏览」与「按压制组筛选」在同一下拉里打架；
--   ③ source 维度实际词表是音乐口径（CD 抓轨/黑胶转录/流媒体/网络发行），
--      纪录片包挂在它上面，值域对不上；
--   ④ 纪录片区别于剧情片的两个核心分面——**解说（narrator）**与
--      **题材/手法（doc_type：观察式/说明式/调查式/历史/传记/社会议题）**
--      ——库里没有承载；
--   ⑤ 获奖（award）是纪录片站常见的荣誉分面（奥斯卡最佳纪录长片/
--      圣丹斯/IDFA 等），无承载。
--
-- 本迁移用既有六类型字段系统补齐，**零业务代码**（维度注册即生效：
-- section_where → typed_pred 按 section_kinds.field_type 分派，见
-- torrents/section_pred.rs + torrents/section_filter.rs）。
--
-- 新增 kind：
--   network   multiselect  出品方/播出机构（BBC/NHK/PBS/国家地理/央视纪录…）
--   doc_type  select       题材/手法（观察式/说明式/调查式/历史/传记/社会议题…）
--   award     text         获奖（自由值：奥斯卡最佳纪录长片…）
-- 复用既有 kind：
--   standard  select       画质档（与 movie 同 kind，词表按纪录片口径配）
--   narrator  text         解说（0315 为 ebook 建，纪录片复用同一 kind）
--
-- ⚠️ 实现纪律（本迁移踩过的坑，务必保留）：
--   · `jsonb_set(obj, '{a,b}', v, true)` 的 `true` 只控制「值已存在时是否
--     替换」，**父路径不存在时不会创建中间层级**——直接返回原对象。
--     故不能靠链式 jsonb_set 累积 `{dict,xxx}`；本迁移改用 jsonb_build_object
--     一次性构造 sections（kinds 数组 + dict 对象），彻底规避。
--   · `jsonb_agg(...)` 在过滤后**无命中时返回 NULL**，`NULL || '{...}'::jsonb`
--     仍为 NULL。故 kinds 重建必须 `COALESCE(jsonb_agg(...), '[]'::jsonb) || …`，
--     否则 jsonb_set 写入 NULL 会把整个 sections 抹成 NULL。
--
-- 幂等：
--   ① section_kinds  ON CONFLICT (kind) DO NOTHING（不覆盖站长已改的 label/sort；
--      field_type 不可改是既有纪律）；
--   ② section_dict   按 (kind,name) NOT EXISTS 追加；
--   ③ documentary 包 sections 按本迁移声明**整体重建**（kinds/dict 皆全量声明，
--      重跑产出一致）。
-- ⚠️ kind 名用小写 [a-z0-9_]（section_kinds 既有约束）。

BEGIN;

-- ============================================================
-- ① 维度定义（六类型：text/number/select/multiselect/date/bool）
-- ============================================================
INSERT INTO section_kinds
    (kind, label, sort, field_type, required, multiple, enabled)
VALUES
  -- 出品方/播出机构：纪录片核心分面（对标 MVGroup/PBS/Nature 系「Network」）
  ('network',  '出品方',  30, 'multiselect', FALSE, FALSE, TRUE),
  -- 题材/手法：纪录片区别于剧情片的类型分面（电影学「纪录片模式」）
  ('doc_type', '题材类型', 35, 'select',     FALSE, FALSE, TRUE),
  -- 获奖：荣誉分面（电影节/奖项自由值）
  ('award',    '获奖',    98, 'text',        FALSE, FALSE, TRUE),
  -- 画质档：与 movie 同 kind（documentary 此前漏挂，本批补上）
  ('standard', '规格',    60, 'select',      FALSE, FALSE, TRUE)
ON CONFLICT (kind) DO NOTHING;

-- ============================================================
-- ② 维度词表（表级：让 /section-dict 在各站型下都有兜底；包 apply 会按包重建）
-- ============================================================

-- 出品方（network）：国际纪录片厂牌 + 公共广播 + 流媒体出品 + 华语机构
INSERT INTO section_dict (kind, name, sort)
SELECT 'network', v.n, v.s FROM (VALUES
  ('BBC',              10),
  ('NHK',              20),
  ('PBS',              30),
  ('National Geographic', 40),
  ('Discovery',        50),
  ('History Channel',  60),
  ('ZDF',              70),
  ('Arte',             80),
  ('Netflix',          90),
  ('Apple TV+',       100),
  ('央视纪录',         110),
  ('CCTV-9',          120),
  ('B站出品',          130),
  ('腾讯视频',         140)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='network' AND d.name=v.n);

-- 题材/手法（doc_type）：纪录片模式（电影学分类）+ 常见题材
INSERT INTO section_dict (kind, name, sort)
SELECT 'doc_type', v.n, v.s FROM (VALUES
  ('观察式',   10),
  ('说明式',   20),
  ('调查式',   30),
  ('参与式',   40),
  ('反身式',   50),
  ('历史',     60),
  ('传记',     70),
  ('自然',     80),
  ('科技',     90),
  ('社会议题', 100),
  ('战争/军事',110),
  ('人文/民俗',120)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='doc_type' AND d.name=v.n);

-- 画质档（standard）：纪录片口径（与 movie 词表分开配）
INSERT INTO section_dict (kind, name, sort)
SELECT 'standard', v.n, v.s FROM (VALUES
  ('2160p/4K', 10),
  ('1080p',    20),
  ('720p',     30),
  ('SD/480p',  40)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='standard' AND d.name=v.n);

-- ============================================================
-- ③ documentary 包 sections：整体重建（kinds + dict 一次性构造）
--    规避 jsonb_set 不创父路径 / jsonb_agg 空集返 NULL 两个坑。
-- ============================================================
UPDATE site_type_packs SET sections = jsonb_build_object(
  'kinds', '[
    {"kind":"media","label":"媒介","sort":10,"field_type":"select"},
    {"kind":"network","label":"出品方","sort":30,"field_type":"multiselect"},
    {"kind":"doc_type","label":"题材类型","sort":35,"field_type":"select"},
    {"kind":"codec","label":"编码","sort":40,"field_type":"select"},
    {"kind":"standard","label":"规格","sort":60,"field_type":"select"},
    {"kind":"season","label":"季","sort":62,"field_type":"select"},
    {"kind":"source","label":"来源","sort":70,"field_type":"select"},
    {"kind":"episode_first","label":"起始集","sort":75,"field_type":"number"},
    {"kind":"episode_last","label":"结束集","sort":85,"field_type":"number"},
    {"kind":"team","label":"制作组","sort":90,"field_type":"select"},
    {"kind":"subtitle_group","label":"字幕组","sort":95,"field_type":"multiselect"},
    {"kind":"narrator","label":"解说","sort":96,"field_type":"text"},
    {"kind":"award","label":"获奖","sort":98,"field_type":"text"}
  ]'::jsonb,
  'dict', jsonb_build_object(
    'media', '["纪录片","短片","影像资料","图文"]'::jsonb,
    'network', '["BBC","NHK","PBS","National Geographic","Discovery",
      "History Channel","ZDF","Arte","Netflix","Apple TV+","央视纪录",
      "CCTV-9","B站出品","腾讯视频"]'::jsonb,
    'doc_type', '["观察式","说明式","调查式","参与式","反身式","历史","传记",
      "自然","科技","社会议题","战争/军事","人文/民俗"]'::jsonb,
    'codec', '["H.264/x264","H.265/x265","AV1","ProRes"]'::jsonb,
    'standard', '["2160p/4K","1080p","720p","SD/480p"]'::jsonb,
    'season', '["第一季","第二季","第三季","第四季","最终季","番外/迷你剧"]'::jsonb,
    'source', '["电视台","流媒体","院线","自制","出版"]'::jsonb,
    'team', '["官方","字幕组","压制组"]'::jsonb,
    'subtitle_group', '["CHDBits","OurBits","HDHome","MTeam","HDSky","无字幕"]'::jsonb
  )
)
WHERE code='documentary';

COMMIT;
