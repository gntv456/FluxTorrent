-- 0223: 站型包 apply 台账（Round10 G21）：快照·回看·回滚
--
-- 现状缺口：/admin/site-type-packs/apply 一次性物化分类/模块/维度/标签/等级/
-- 经济/元数据/术语/字幕口径，站长切错站型只能凭记忆手工改回。
-- 0167 内容包给过先例：「回滚 = 把导入前快照当一次导入执行」——站型包同款：
--   1) site_pack_applies：每次 apply 落一条（diff 预览 / 计数 / 应用前全量快照）；
--   2) apply_pack_extras_json(pack jsonb)：回滚的临时包不在 site_type_packs 表里，
--      原 apply_pack_extras(code) 按 code 查表会 RAISE，故拆出「吃载荷」版，
--      原函数改为查表转交（行为不变，调用点无需改）。
-- 回滚执行链在 API 侧（staff_http/pack_applies.rs）。

CREATE TABLE IF NOT EXISTS site_pack_applies (
    id BIGSERIAL PRIMARY KEY,
    pack_code TEXT NOT NULL,
    pack_name TEXT NOT NULL,
    mode TEXT NOT NULL,
    -- apply 将改动的键旧值→新值（回看；与向导 diff 预览同源）
    changes JSONB NOT NULL DEFAULT '[]'::jsonb,
    -- 回执计数：categories/extras/terms
    counts JSONB NOT NULL DEFAULT '{}'::jsonb,
    -- 应用前全量状态（分类树含层级/排序、站型/站名/标语/字幕口径、模块开关、
    -- sections/tags/classes/economy/metadata/terms 七段载荷）
    snapshot JSONB NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    applied_by INT REFERENCES users(id) ON DELETE SET NULL,
    rolled_back_at TIMESTAMPTZ,
    rolled_back_by INT REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_site_pack_applies_time
    ON site_pack_applies (applied_at DESC, id DESC);

-- extras 应用核心改为「吃载荷」：键缺失/JSON null/类型不符一律视为未声明
CREATE OR REPLACE FUNCTION apply_pack_extras_json(pack jsonb)
RETURNS TABLE(kind text, applied bigint)
LANGUAGE plpgsql AS $$
BEGIN
  IF pack IS NULL THEN
    RAISE EXCEPTION 'apply_pack_extras_json: pack payload required';
  END IF;

  -- 等级叙事：只改名称列，阈值/规则不动（§11.2）
  IF jsonb_typeof(pack->'classes') = 'array' THEN
    RETURN QUERY SELECT 'classes'::text, count(*)::bigint
      FROM jsonb_to_recordset(pack->'classes') AS c(id int, name text);
    UPDATE class_rules cr SET name = c.name
    FROM jsonb_to_recordset(pack->'classes') AS c(id int, name text)
    WHERE cr.class_id = c.id;
  END IF;

  -- 经济预设：仅写已登记的 settings_meta 键（防塞任意键）
  IF jsonb_typeof(pack->'economy') = 'object' THEN
    RETURN QUERY SELECT 'economy'::text, count(*)::bigint
      FROM jsonb_each(pack->'economy') e(k, v)
      WHERE EXISTS (SELECT 1 FROM settings_meta m WHERE m.name = e.k);
    INSERT INTO site_settings (name, value)
    SELECT e.k, e.v #>> '{}'
    FROM jsonb_each(pack->'economy') e(k, v)
    WHERE EXISTS (SELECT 1 FROM settings_meta m WHERE m.name = e.k)
    ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now();
  END IF;

  -- 元数据源默认
  IF jsonb_typeof(pack->'metadata'->'sources') = 'array' THEN
    RETURN QUERY SELECT 'metadata'::text, 1::bigint;
    INSERT INTO site_settings (name, value)
    VALUES ('metadata_sources',
            (SELECT string_agg(s, ',')
             FROM jsonb_array_elements_text(pack->'metadata'->'sources') s))
    ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now();
  END IF;
END $$;

-- 原入口保持行为不变：按 code 查表 → 转交载荷版
CREATE OR REPLACE FUNCTION apply_pack_extras(pack_code text)
RETURNS TABLE(kind text, applied bigint)
LANGUAGE plpgsql AS $$
DECLARE
  p jsonb;
BEGIN
  SELECT to_jsonb(t) INTO p FROM site_type_packs t WHERE code = pack_code;
  IF p IS NULL THEN RAISE EXCEPTION 'pack % not found', pack_code; END IF;
  RETURN QUERY SELECT * FROM apply_pack_extras_json(p);
END $$;
