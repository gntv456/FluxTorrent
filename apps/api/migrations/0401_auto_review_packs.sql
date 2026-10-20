-- 0401 自动审种批 1.5/2/3/4（站型出厂包 + 预设映射 + 台账回填与降级登记）。
--
-- 依据：《自动审种功能策划案》v1.3 §2.5（站型适配层）/§3.1（卡片 2 一键
-- 预设）/§6（熔断与规则降级）/§12（申诉推翻降级台账）。
--
-- 全部幂等（ON CONFLICT DO NOTHING / IF NOT EXISTS），重跑安全。

-- ============ ① 站型出厂包（§2.5 覆盖矩阵） ============
-- 套用语义（review_auto 的 apply_site_pack）：把 overrides 里的键值写进
-- site_settings 同名键（站长已改与否以 settings_meta.readonly 标记区分——
-- 本表自带 changed_at，套用时站长改过的键不覆盖，见 §18.5 升级安全）。
CREATE TABLE IF NOT EXISTS auto_review_site_packs (
  site_type  TEXT PRIMARY KEY,
  pack_ver   INT NOT NULL DEFAULT 1,
  -- 该站型的标题命名模板缺省（§2.6；站长可在卡片 6 再改）
  title_template TEXT NOT NULL DEFAULT '',
  -- 覆盖项：{"arule_dupe_check_action": "off", ...}（键=site_settings.name）
  overrides  JSONB NOT NULL DEFAULT '{}'::jsonb,
  changed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO auto_review_site_packs (site_type, title_template, overrides) VALUES
  ('movie', '{中文名}.{年份}.{来源}.{分辨率}-{组名}',
   '{"arule_dupe_check_threshold": "88"}'::jsonb),
  ('tv', '{中文名}.S{季}E{话}.{来源}.{分辨率}-{组名}',
   '{"arule_descr_missing_min_chars": "40"}'::jsonb),
  ('anime', '{中文名} 第{话数}话 [{字幕组}]',
   '{"arule_descr_missing_min_chars": "40"}'::jsonb),
  ('music', '{艺人} - {专辑} ({年份}) [{音质}]',
   '{"arule_descr_missing_min_chars": "30"}'::jsonb),
  ('sports', '{联赛} {赛季} {主队} vs {客队} {日期} {来源}',
   '{"arule_dupe_check_enabled": "0"}'::jsonb),
  ('documentary', '{中文名}.{年份}', '{}'::jsonb),
  ('education', '{中文名}.{格式}', '{}'::jsonb),
  ('ebook', '{书名} ({作者}) [{格式}]',
   '{"arule_descr_missing_enabled": "0"}'::jsonb),
  ('comic', '{中文名} 第{话数}话 [{字幕组}]',
   '{"arule_descr_missing_min_chars": "30"}'::jsonb),
  ('variety', '{中文名}.{年份}.{期数}',
   '{"arule_dupe_check_threshold": "90"}'::jsonb)
ON CONFLICT (site_type) DO NOTHING;

COMMENT ON TABLE auto_review_site_packs IS
  '自动审种站型出厂包：site_type → 命名模板缺省 + 规则覆盖（套用永不覆盖站长已改键）';

-- ============ ② 台账回填列（§13.1 人机相遇对照 / 人工终审回填） ============
ALTER TABLE auto_review_log ADD COLUMN IF NOT EXISTS
  human_decided_at TIMESTAMPTZ;

-- 「人工推翻自动裁决」的降级台账（§6/§12）：按规则累计，≥10 例自动降级
CREATE TABLE IF NOT EXISTS auto_review_rule_health (
  rule        TEXT PRIMARY KEY,
  overturns   INT NOT NULL DEFAULT 0,      -- 人工推翻（裁决被改判）
  appeal_wins INT NOT NULL DEFAULT 0,      -- 申诉翻案（权重×2 进同计数）
  demoted_at  TIMESTAMPTZ,                 -- 自动降级为「仅暂缓」的时间
  updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE auto_review_rule_health IS
  '自动审种规则健康度：推翻/申诉翻案累计；appeal_wins 计双倍权重，≥10 自动降级仅暂缓';

-- ============ ③ 预设→键值映射（卡片 2 一键带出的服务端真源） ============
-- settings 保存链（PUT /admin/settings/groups）直接写这些键；本表只做
-- 「选预设=批量带出」的映射源（review_auto 的 apply_preset 读它）。
CREATE TABLE IF NOT EXISTS auto_review_presets (
  preset     TEXT PRIMARY KEY,             -- balanced / cautious / efficient / off
  label_zh   TEXT NOT NULL,
  overrides  JSONB NOT NULL DEFAULT '{}'::jsonb
);

INSERT INTO auto_review_presets (preset, label_zh, overrides) VALUES
  ('balanced', '均衡（出厂缺省）',
   '{"auto_review_mode": "shadow", "arule_title_garbled_action": "deny",
     "arule_ads_keywords_action": "deny", "arule_leak_marker_action": "deny",
     "arule_descr_missing_action": "postpone", "arule_dupe_check_action": "postpone",
     "arule_title_format_action": "postpone", "arule_title_format_level": "warn"}'::jsonb),
  ('cautious', '谨慎（几乎不自动拒）',
   '{"auto_review_mode": "half", "arule_title_garbled_action": "postpone",
     "arule_ads_keywords_action": "postpone", "arule_leak_marker_action": "deny",
     "arule_descr_missing_action": "postpone", "arule_dupe_check_action": "postpone",
     "arule_title_format_action": "postpone", "arule_title_format_level": "warn"}'::jsonb),
  ('efficient', '高效（激进减负）',
   '{"auto_review_mode": "full", "arule_title_garbled_action": "deny",
     "arule_ads_keywords_action": "deny", "arule_leak_marker_action": "deny",
     "arule_descr_missing_action": "postpone", "arule_dupe_check_action": "deny",
     "arule_title_format_action": "postpone", "arule_title_format_level": "strict"}'::jsonb),
  ('off', '关闭（全部人工）',
   '{"auto_review_enabled": "0"}'::jsonb)
ON CONFLICT (preset) DO NOTHING;

COMMENT ON TABLE auto_review_presets IS '自动审种一键预设的键值映射真源（卡片 2）';
