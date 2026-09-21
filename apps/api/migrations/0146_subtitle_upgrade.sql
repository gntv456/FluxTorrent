-- 0146: 字幕区改造（策划案 _doc/字幕区改造开发策划方案.md）
-- B1+B2+B3 数据模型一次落齐：元数据/治理字段、语言字典、评分表、求字幕悬赏表、
-- 设置项与后台元信息、lyric 口径预设。代码分批接线，未用列不影响旧链路。
--
-- 注意：所有新端点必须落在 /api/v1/subtitles 前缀下（gateway.rs:38 只登记了它）。

-- ============ 1) subtitles 元数据与治理字段 ============
ALTER TABLE subtitles
  ADD COLUMN IF NOT EXISTS size BIGINT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS ext TEXT,
  ADD COLUMN IF NOT EXISTS fps NUMERIC(6,3),
  ADD COLUMN IF NOT EXISTS machine_translated BOOLEAN NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS hearing_impaired BOOLEAN NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS foreign_parts_only BOOLEAN NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS source TEXT,          -- 原创翻译/校订翻译/市售DVD/官方译本/其他（lyric: 原创听译/官方歌词本/转载）
  ADD COLUMN IF NOT EXISTS producer TEXT,
  ADD COLUMN IF NOT EXISTS proofreader TEXT,
  ADD COLUMN IF NOT EXISTS author_name TEXT,     -- 译者/字幕组署名（KG "Subtitles: A and B for KG" 同款）
  ADD COLUMN IF NOT EXISTS parent_id BIGINT REFERENCES subtitles(id),   -- 修订版
  ADD COLUMN IF NOT EXISTS release_name TEXT,    -- 归一化 scene release name，用于匹配
  ADD COLUMN IF NOT EXISTS lang_id SMALLINT,
  ADD COLUMN IF NOT EXISTS anon BOOLEAN NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS verified BOOLEAN NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS status SMALLINT NOT NULL DEFAULT 1,          -- 0 pending / 1 approved / 2 rejected
  ADD COLUMN IF NOT EXISTS moderated_by BIGINT REFERENCES users(id),
  ADD COLUMN IF NOT EXISTS moderated_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS rating_sum INT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS rating_count INT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS bad_reports INT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;

-- ============ 2) 语言字典表（替代前端 31 项硬编码） ============
CREATE TABLE IF NOT EXISTS subtitle_langs (
  id SMALLSERIAL PRIMARY KEY,
  code TEXT UNIQUE NOT NULL,
  name TEXT NOT NULL,
  flag TEXT,
  position INT NOT NULL DEFAULT 0
);
INSERT INTO subtitle_langs (code, name, flag, position) VALUES
  ('chs','简体中文','🇨🇳',1), ('cht','繁體中文','🇹🇼',2), ('eng','English','🇬🇧',3),
  ('jpn','日本語','🇯🇵',4), ('kor','한국어','🇰🇷',5), ('fre','Français','🇫🇷',6),
  ('ger','Deutsch','🇩🇪',7), ('spa','Español','🇪🇸',8), ('rus','Русский','🇷🇺',9),
  ('tha','ไทย','🇹🇭',10), ('vie','Tiếng Việt','🇻🇳',11), ('ita','Italiano','🇮🇹',12),
  ('por','Português','🇵🇹',13), ('nld','Nederlands','🇳🇱',14), ('swe','Svenska','🇸🇪',15),
  ('dan','Dansk','🇩🇰',16), ('fin','Suomi','🇫🇮',17), ('nor','Norsk','🇳🇴',18),
  ('pol','Polski','🇵🇱',19), ('cze','Čeština','🇨🇿',20), ('hun','Magyar','🇭🇺',21),
  ('rum','Română','🇷🇴',22), ('bul','Български','🇧🇬',23), ('hrv','Hrvatski','🇭🇷',24),
  ('srp','Српски','🇷🇸',25), ('slk','Slovenčina','🇸🇰',26), ('slv','Slovenščina','🇸🇮',27),
  ('ell','Ελληνικά','🇬🇷',28), ('heb','עברית','🇮🇱',29), ('tur','Türkçe','🇹🇷',30),
  ('ara','العربية','🇸🇦',31), ('hin','हिन्दी','🇮🇳',32), ('ind','Indonesia','🇮🇩',33),
  ('may','Bahasa Melayu','🇲🇾',34), ('other','其他','🌐',99)
ON CONFLICT (code) DO NOTHING;

-- ============ 3) 历史回填 ============
UPDATE subtitles s SET size = a.size
  FROM attachments a
 WHERE s.file_ref = 'attach://' || a.sha256 AND s.size = 0;

UPDATE subtitles s SET lang_id = l.id
  FROM subtitle_langs l WHERE s.lang = l.code AND s.lang_id IS NULL;

UPDATE subtitles s SET release_name = lower(regexp_replace(t.name, '[^a-zA-Z0-9]', '', 'g'))
  FROM torrents t WHERE t.id = s.torrent_id AND s.release_name IS NULL;

CREATE INDEX IF NOT EXISTS idx_subtitles_release ON subtitles (release_name);
CREATE INDEX IF NOT EXISTS idx_subtitles_torrent ON subtitles (torrent_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_subtitles_lang ON subtitles (lang_id);
CREATE INDEX IF NOT EXISTS idx_subtitles_sha ON subtitles ((file_ref)) WHERE file_ref LIKE 'attach://';

-- ============ 4) 评分表 / 求字幕悬赏表 ============
CREATE TABLE IF NOT EXISTS subtitle_votes (
  subtitle_id BIGINT NOT NULL REFERENCES subtitles(id) ON DELETE CASCADE,
  user_id BIGINT NOT NULL REFERENCES users(id),
  score INT NOT NULL CHECK (score BETWEEN 1 AND 10),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (subtitle_id, user_id)
);

-- KG pots 口径：发起（lang + 可选 torrent_id）→ 众人 contribute 凑魔力入池 →
-- 译者上传并认领 fulfill → 发起人/管理确认 → 整池一次性结算（幂等键锚定请求）
CREATE TABLE IF NOT EXISTS subtitle_requests (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id),
  torrent_id BIGINT REFERENCES torrents(id),
  lang TEXT NOT NULL,
  descr TEXT,
  bounty BIGINT NOT NULL DEFAULT 0,
  contributors JSONB NOT NULL DEFAULT '[]',    -- [{user_id,name,amount}]
  status SMALLINT NOT NULL DEFAULT 0,          -- 0 进行中 / 1 已交付 / 2 已撤销
  fulfilled_subtitle_id BIGINT REFERENCES subtitles(id),
  paid_at TIMESTAMPTZ,                         -- 整池结算时间（幂等核对用）
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============ 5) 设置项 ============
INSERT INTO site_settings (name, value, descr) VALUES
  ('subtitle_moderation', '0', '字幕是否需要审核（0 发布即生效 / 1 需审核）'),
  ('subtitle_bad_threshold', '3', '字幕被标记坏字幕达此数自动隐藏'),
  ('subtitle_ext_whitelist', 'srt,ass,ssa,sup,idx,sub,cue,zip,rar,7z', '允许上传的字幕扩展名（逗号分隔，kind=lyric 时另设）'),
  ('subtitle_kind', 'subtitle', '字幕区口径：subtitle 影视字幕 / lyric 歌词'),
  ('subtitle_label', '字幕', '字幕区显示名（导航与页面标题）'),
  ('subtitle_show_fps', '1', '是否显示 FPS 字段（歌词无需）')
ON CONFLICT (name) DO NOTHING;

-- 歌词口径覆盖（音乐/无损站型当前值；站长后续改的值优先生效——本迁移只在键缺省时播种）
UPDATE site_settings SET value = 'lrc,srt,ass,zip'
 WHERE name = 'subtitle_ext_whitelist'
   AND (SELECT value FROM site_settings WHERE name = 'site_type') IN ('music','lossless');
UPDATE site_settings SET value = 'lyric'
 WHERE name = 'subtitle_kind'
   AND (SELECT value FROM site_settings WHERE name = 'site_type') IN ('music','lossless');
UPDATE site_settings SET value = '歌词'
 WHERE name = 'subtitle_label'
   AND (SELECT value FROM site_settings WHERE name = 'site_type') IN ('music','lossless');
UPDATE site_settings SET value = '0'
 WHERE name = 'subtitle_show_fps'
   AND (SELECT value FROM site_settings WHERE name = 'site_type') IN ('music','lossless');

-- ============ 6) 站型包 modules 矩阵补齐（§12.1：影视/动漫/综合显式开） ============
UPDATE site_type_packs
   SET modules = modules::jsonb || '{"subtitles":true}'::jsonb
 WHERE code IN ('movie','documentary','anime','general');

-- ============ 7) 后台元信息（照 0039 分块 INSERT 写法，列数严格一致） ============
INSERT INTO settings_meta (name, type, label_zh, label_en, options, group_key, card_order) VALUES
  ('subtitle_moderation', 'yesno', '字幕需要审核', 'Subtitle moderation', NULL, '上传限制', 10),
  ('subtitle_kind', 'enum', '字幕区口径', 'Subtitle kind',
   '{"options": ["subtitle", "lyric"]}'::jsonb, '基础信息', 95),
  ('subtitle_label', 'text', '字幕区显示名', 'Subtitle label', NULL, '基础信息', 96),
  ('subtitle_show_fps', 'yesno', '显示 FPS 字段', 'Show FPS field', NULL, '基础信息', 97)
ON CONFLICT (name) DO UPDATE
  SET type = EXCLUDED.type, label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
      options = EXCLUDED.options, group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;

INSERT INTO settings_meta (name, type, label_zh, label_en, unit, min, max, step, group_key, card_order) VALUES
  ('subtitle_bad_threshold', 'number', '坏字幕隐藏阈值', 'Bad subtitle threshold', '次', 1, 100, 1, '上传限制', 11),
  ('subtitle_ext_whitelist', 'text', '字幕扩展名白名单', 'Subtitle extensions', NULL, NULL, NULL, 1, '上传限制', 12)
ON CONFLICT (name) DO UPDATE
  SET type = EXCLUDED.type, label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
      unit = EXCLUDED.unit, min = EXCLUDED.min, max = EXCLUDED.max, step = EXCLUDED.step,
      group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;

-- ============ 8) 切站型联动（pack_apply 调用；站长手动改过的值仍会被覆盖，
--    与 module_* 同口径：apply = 显式重置站型默认，0145 tagline 教训不适用于
--    「决定校验规则」的键——kind 与白名单不一致会导致上传全部被拒） ============
CREATE OR REPLACE FUNCTION apply_subtitle_kind(site_code text) RETURNS void
LANGUAGE plpgsql AS $$
DECLARE
  is_lyric boolean;
BEGIN
  is_lyric := site_code IN ('music','lossless');
  INSERT INTO site_settings (name, value, descr) VALUES
    ('subtitle_kind', CASE WHEN is_lyric THEN 'lyric' ELSE 'subtitle' END,
     '字幕区口径：subtitle 影视字幕 / lyric 歌词'),
    ('subtitle_label', CASE WHEN is_lyric THEN '歌词' ELSE '字幕' END,
     '字幕区显示名（导航与页面标题）'),
    ('subtitle_ext_whitelist',
     CASE WHEN is_lyric THEN 'lrc,srt,ass,zip' ELSE 'srt,ass,ssa,sup,idx,sub,cue,zip,rar,7z' END,
     '允许上传的字幕扩展名（逗号分隔）'),
    ('subtitle_show_fps', CASE WHEN is_lyric THEN '0' ELSE '1' END,
     '是否显示 FPS 字段（歌词无需）')
  ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now();
END $$;
