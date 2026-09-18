-- 0108: 通用建站系统 U2 —— 站型包做实（策划案 §7/§11）
--
-- 1) 站型包携带等级叙事（§11.2）：classes JSONB，apply 时更新 class_rules 名称
--    （晋升数值阈值保留全站默认——升降级规则跨站型通用，只换叙事皮肤）。
-- 2) 站型包携带经济预设与元数据源默认（§7.1）：economy/metadata_sources JSONB。
-- 3) demo 数据清理过程（§11.1）：registration 完成向导后调用（U3 接 setup_done）。
-- 4) 访客策略（§11.6）：guest_policy = all_private（默认，现状）/ recent_only / showcase。

-- ============ 1) site_type_packs 扩列 ============

ALTER TABLE site_type_packs ADD COLUMN IF NOT EXISTS classes JSONB;          -- [{id,name}] 可选：等级叙事皮肤
ALTER TABLE site_type_packs ADD COLUMN IF NOT EXISTS economy JSONB;          -- {key:value} 可选：经济预设（一次性写入）
ALTER TABLE site_type_packs ADD COLUMN IF NOT EXISTS metadata JSONB;         -- {sources:[...]} 可选：元数据源默认

-- 等级叙事：教育保留植物系；综合/其余给中性 LV 命名（站长可改）
UPDATE site_type_packs SET classes = $$
[
  {"id":1,"name":"LV1"}, {"id":2,"name":"LV2"}, {"id":3,"name":"LV3"}, {"id":4,"name":"LV4"},
  {"id":5,"name":"LV5"}, {"id":6,"name":"LV6"}, {"id":7,"name":"LV7"}, {"id":8,"name":"LV8"},
  {"id":9,"name":"LV9"}, {"id":10,"name":"LV10"}, {"id":11,"name":"LV11"}, {"id":12,"name":"LV12"}
]$$::jsonb
WHERE code IN ('general','movie','music','anime','ebook','sports','game','software','documentary','lossless');

UPDATE site_type_packs SET classes = $$
[
  {"id":1,"name":"LV1 新芽"}, {"id":2,"name":"LV2 幼苗"}, {"id":3,"name":"LV3 小苗"}, {"id":4,"name":"LV4 小树"},
  {"id":5,"name":"LV5 幼树"}, {"id":6,"name":"LV6 乔木"}, {"id":7,"name":"LV7 大树"}, {"id":8,"name":"LV8 花蕾"},
  {"id":9,"name":"LV9 开花"}, {"id":10,"name":"LV10 结果"}, {"id":11,"name":"LV11 硕果"}, {"id":12,"name":"LV12 森林"}
]$$::jsonb
WHERE code = 'education';

-- 经济预设（§7.1：一次性写入，之后归设置中心管；键须已存在于 settings_meta）
UPDATE site_type_packs SET economy = $$
{ "attendance_first": 10, "attendance_streak": 5, "bank_max_rate_pct": 18 }
$$::jsonb WHERE code = 'education';

-- 元数据源预设（§7.1）：影视 imdb+douban / 音乐类 douban+musicbrainz 口径
UPDATE site_type_packs SET metadata = $${"sources":["imdb","douban"]}$$::jsonb WHERE code IN ('movie','documentary');
UPDATE site_type_packs SET metadata = $${"sources":["douban","musicbrainz"]}$$::jsonb WHERE code IN ('music','lossless');

-- ============ 2) 访客策略（§11.6）============

INSERT INTO site_settings (name, value, descr, grp) VALUES
('guest_policy', 'all_private', '访客可见性：all_private / recent_only / showcase', 'main')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, options, group_key, card_order) VALUES
('guest_policy', 'enum', '访客可见性', 'Guest Policy',
 '{"options": ["all_private", "recent_only", "showcase"]}'::jsonb, 'basic', 91)
ON CONFLICT (name) DO UPDATE
  SET type = EXCLUDED.type, label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
      options = EXCLUDED.options, group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;

-- ============ 3) demo 清理过程（§11.1：U3 setup_done 置位时调用；此处先建好）============

CREATE OR REPLACE FUNCTION purge_demo_data() RETURNS TABLE(kind text, removed bigint)
LANGUAGE plpgsql AS $$
BEGIN
  -- demo 用户口径：0018 迁移的 12 名 @demo.local 账号（密码统一 password123）
  RETURN QUERY
    SELECT 'users'::text, count(*)::bigint FROM users WHERE email LIKE '%@demo.local';
  DELETE FROM users WHERE email LIKE '%@demo.local';
  -- demo 种子口径：0018/0022 的演示种子标题前缀「[demo]」
  RETURN QUERY
    SELECT 'torrents'::text, count(*)::bigint FROM torrents WHERE title LIKE '[demo]%';
  DELETE FROM torrents WHERE title LIKE '[demo]%';
  RETURN QUERY
    SELECT 'comments'::text, count(*)::bigint FROM comments c
    WHERE NOT EXISTS (SELECT 1 FROM torrents t WHERE t.id = c.torrent_id);
  RETURN QUERY
    SELECT 'invites'::text, count(*)::bigint FROM invites i
    WHERE NOT EXISTS (SELECT 1 FROM users u WHERE u.id = i.inviter_id);
  RETURN QUERY
    SELECT 'forum_posts'::text, count(*)::bigint FROM forum_posts p
    WHERE NOT EXISTS (SELECT 1 FROM users u WHERE u.id = p.user_id);
END $$;

-- ============ 4) apply 过程强化（供 API 复用；classes/economy/metadata 落库）============

CREATE OR REPLACE FUNCTION apply_pack_extras(pack_code text) RETURNS TABLE(kind text, applied bigint)
LANGUAGE plpgsql AS $$
DECLARE
  pack record;
BEGIN
  SELECT * INTO pack FROM site_type_packs WHERE code = pack_code;
  IF NOT FOUND THEN RAISE EXCEPTION 'pack % not found', pack_code; END IF;

  -- 等级叙事：只改名称列，阈值/规则不动（§11.2）
  IF pack.classes IS NOT NULL THEN
    RETURN QUERY SELECT 'classes'::text, count(*)::bigint FROM jsonb_to_recordset(pack.classes) AS c(id int, name text);
    UPDATE class_rules cr SET name = c.name
    FROM jsonb_to_recordset(pack.classes) AS c(id int, name text)
    WHERE cr.class_id = c.id;
  END IF;

  -- 经济预设：仅写已登记的 settings_meta 键（防塞任意键）
  IF pack.economy IS NOT NULL THEN
    RETURN QUERY SELECT 'economy'::text, count(*)::bigint FROM jsonb_each(pack.economy) e(k, v)
      WHERE EXISTS (SELECT 1 FROM settings_meta m WHERE m.name = e.k);
    INSERT INTO site_settings (name, value)
    SELECT e.k, e.v #>> '{}'
    FROM jsonb_each(pack.economy) e(k, v)
    WHERE EXISTS (SELECT 1 FROM settings_meta m WHERE m.name = e.k)
    ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now();
  END IF;

  -- 元数据源默认
  IF pack.metadata ? 'sources' THEN
    RETURN QUERY SELECT 'metadata'::text, 1::bigint;
    INSERT INTO site_settings (name, value)
    VALUES ('metadata_sources', (SELECT string_agg(s, ',') FROM jsonb_array_elements_text(pack.metadata->'sources') s))
    ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now();
  END IF;
END $$;
