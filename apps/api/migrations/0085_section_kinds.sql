-- 0085: 质量维度可配置化（NP 自定义 Section 口径）
-- 站方可自定义质量维度（kind）：影视站可加「分辨率/语种」，教育站保留「学段/版本」。
-- section_dict.kind 从枚举 CHECK 放开为外键引用 section_kinds，字典行随维度级联删除。

CREATE TABLE IF NOT EXISTS section_kinds (
  kind  TEXT PRIMARY KEY,
  label TEXT NOT NULL,
  sort  INT  NOT NULL DEFAULT 0
);

-- 预置既有 9 维：3 个落实体表（media/grades/editions），6 个走 section_dict
INSERT INTO section_kinds (kind, label, sort) VALUES
  ('media',       '媒介',     10),
  ('grades',      '学段',     20),
  ('editions',    '版本',     30),
  ('codec',       '编码',     40),
  ('audio_codec', '音频编码', 50),
  ('standard',    '规格',     60),
  ('source',      '来源',     70),
  ('processing',  '处理工艺', 80),
  ('team',        '制作组',   90)
ON CONFLICT (kind) DO NOTHING;

-- 放开 section_dict.kind 的枚举约束 → 外键到 section_kinds（级联删字典行）
ALTER TABLE section_dict DROP CONSTRAINT IF EXISTS section_dict_kind_check;
ALTER TABLE section_dict ADD CONSTRAINT section_dict_kind_fkey
  FOREIGN KEY (kind) REFERENCES section_kinds(kind) ON DELETE CASCADE;
