-- 0195：B 批承重墙·第一层 —— 内容字段类型系统（数据模型）
--
-- 四审 L3「内容自定义字段」判定**不闭环**：站长的内容侧维度在数据模型上只能表达
-- 「单选枚举」——值必须是 section_dict 里预置的 dict_id，且关联表主键 (torrent_id,kind)
-- 结构上限定一条 ⇒ 文本/数字/日期/多选/布尔一概做不到。而用户侧（0186_user_fields）
-- 早已有完整六类型系统。本迁移把同一套类型语义铺到内容侧（设计稿：
-- `_doc/B批承重墙-内容字段类型系统设计稿-2026-09-25.md`）。
--
-- 三层改动，**全部向后兼容**（存量行零语义变化）：
--   1) section_kinds 扩列：field_type / required / multiple / icon_key / bg_color / enabled
--      —— 默认 field_type='select' ⇒ 存量 9 维行为不变。
--   2) torrent_sections 加 value(JSONB) + ordinal，主键改 (torrent_id,kind,ordinal)
--      —— 存量行 ordinal=0，读取语义与旧完全一致；枚举仍走 dict_id，自由值走 value。
--   3) mode_kinds 关联表（替代 category_modes 的 7 个固定布尔列）
--      —— 旧 7 列**本批保留**，读取侧切关联表；自建维度自此可纳入模式管辖。
--   4) categories 加 sort 列（四审 L4：现状只能 SQL 侧 ORDER BY id）
--
-- ⚠ 主键变更会重写表与索引。开发库规模可忽略；存量站长库执行时长与站点规模成正比，
--   建议低峰窗口升级（表行数 ≈ 种子数 × 平均维度数）。
--
-- 幂等：全部 IF NOT EXISTS / DROP IF EXISTS；重复执行安全。

-- ============ 1) section_kinds 扩列（0085 平行升级；用户侧 0186 的同形语义）============
ALTER TABLE section_kinds
    -- 字段类型：与 0186_user_fields.type 的六类型**逐字对齐**，共用一套校验器
    ADD COLUMN IF NOT EXISTS field_type TEXT NOT NULL DEFAULT 'select',
    ADD COLUMN IF NOT EXISTS required   BOOLEAN NOT NULL DEFAULT FALSE,
    -- 多值开关：枚举维度表示可多选；自由值维度表示可填多值
    ADD COLUMN IF NOT EXISTS multiple   BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS icon_key   TEXT,
    ADD COLUMN IF NOT EXISTS bg_color   TEXT,
    ADD COLUMN IF NOT EXISTS enabled    BOOLEAN NOT NULL DEFAULT TRUE;

-- 类型约束（单独加，便于存量库幂等；取值集合与 user_field_defs.type 一致）
DO $$ BEGIN
    ALTER TABLE section_kinds ADD CONSTRAINT section_kinds_field_type_check
        CHECK (field_type IN ('select','multiselect','text','number','date','bool'));
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

-- 选项集不新增列：枚举选项仍在 section_dict（该表是「行」不是「列」，符合
-- 设计原则 4「可扩展性留在数据行，不留列名」）。

-- ============ 2) torrent_sections 值表改造 ============
-- 自由值列（text/number/date/bool 存标量；多选与复杂值存数组/对象）
ALTER TABLE torrent_sections ADD COLUMN IF NOT EXISTS value jsonb;
-- 多值序号：存量行默认 0，语义与旧单值完全一致
ALTER TABLE torrent_sections ADD COLUMN IF NOT EXISTS ordinal INT NOT NULL DEFAULT 0;

-- 主键 (torrent_id, kind) → (torrent_id, kind, ordinal)
DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_constraint
         WHERE conrelid = to_regclass('torrent_sections')
           AND contype = 'p'
           AND pg_get_constraintdef(oid) = 'PRIMARY KEY (torrent_id, kind)'
    ) THEN
        ALTER TABLE torrent_sections DROP CONSTRAINT torrent_sections_pkey;
        ALTER TABLE torrent_sections ADD PRIMARY KEY (torrent_id, kind, ordinal);
    END IF;
END $$;

-- 枚举行必须有 dict_id，自由值行必须有 value（防写入空行）
ALTER TABLE torrent_sections ALTER COLUMN dict_id DROP NOT NULL;
DO $$ BEGIN
    ALTER TABLE torrent_sections ADD CONSTRAINT torrent_sections_value_or_dict
        CHECK (dict_id IS NOT NULL OR value IS NOT NULL);
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

-- 自由值筛选的索引（站长选定「全类型都支持筛选」，见设计稿开口项 4）：
--   · (kind, ordinal)：多值维度按序号取前 N 条
--   · (kind, value)：自由值等值/包含匹配（B2 的 ILIKE 与范围查询在数值上走 btree）
-- 注：dict_id 侧已有 idx_torrent_sections_dict (kind, dict_id)（0063），不重复建。
CREATE INDEX IF NOT EXISTS idx_torrent_sections_kind_ord
    ON torrent_sections (kind, ordinal);
CREATE INDEX IF NOT EXISTS idx_torrent_sections_kind_value
    ON torrent_sections (kind, value);

-- ============ 3) mode_kinds：(mode_id, kind) 关联表 ============
-- 四审 L4 P1：category_modes 是 7 个固定布尔列，section_public.rs 按 kind 名 match、
-- 其余 `_ => true` ⇒ 站长自建的维度永远不受模式管辖；且这 7 个名字
-- （含 audio_codec/standard/processing）本身就是影视/音频词表进了 schema。
-- 改为关联表后，「哪些维度在某模式下可见」是数据而非列。
CREATE TABLE IF NOT EXISTS mode_kinds (
    mode_id INT  NOT NULL REFERENCES category_modes(id) ON DELETE CASCADE,
    kind    TEXT NOT NULL REFERENCES section_kinds(kind) ON DELETE CASCADE,
    visible BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY (mode_id, kind)
);

-- 从旧 7 列回填（含 visible=false 的行——保留站长的关闭意图）
INSERT INTO mode_kinds (mode_id, kind, visible)
SELECT m.id, k.kind, k.visible
  FROM category_modes m
  CROSS JOIN LATERAL (VALUES
      ('source',      m.show_source),
      ('medium',      m.show_medium),
      ('codec',       m.show_codec),
      ('audio_codec', m.show_audio_codec),
      ('standard',    m.show_standard),
      ('processing',  m.show_processing),
      ('team',        m.show_team)
  ) AS k(kind, visible)
 WHERE EXISTS (SELECT 1 FROM section_kinds sk WHERE sk.kind = k.kind)
ON CONFLICT DO NOTHING;

-- ⚠ 旧 7 个 show_* 列**本批保留**（站长拍板：保留一个发布周期，回滚容易）。
--   读取侧 section_public.rs 切到本表；缺行语义 = 可见（与旧 `_ => true` 一致）。

-- ============ 4) categories.sort（四审 L4 P1：现状无排序列）============
-- 存量行统一 100 ⇒ 相同值下退化为 id 序，行为不变；站长可自行调出想要的前后。
ALTER TABLE categories ADD COLUMN IF NOT EXISTS sort INT NOT NULL DEFAULT 100;
CREATE INDEX IF NOT EXISTS idx_categories_sort ON categories (sort, id);
