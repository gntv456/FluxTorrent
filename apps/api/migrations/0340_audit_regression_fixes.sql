-- 0340（v0.3.2 上线后全面回归审计修复批，2026-10-09）
--
-- 四组收口，全部幂等：
-- ① season 维度 select/text 全局翻转修复（0320 升 select → 0324 为 sports
--    又全局降回 text，anime/movie/documentary 三包声明的季度下拉词表因此
--    挂不上）。拆分口径：anime/movie/documentary 用 season（select，
--    第X季词表），sports 改走独立 kind season_label（text，自由赛季
--    “2025-26”）。只动 section_kinds 与词表，不搬 torrent_sections 存量
--    （sports 站的 season 行保持原 kind，检索不受影响——text 谓词是
--    LIKE，select 谓词 dict_id 等值，二者对同一 kind 并存时按声明类型走）。
-- ② 0338 锚点通用化补回填：存量 author/studio 维度值此前没有迁移回填，
--    只有发种/编辑时的增量同步（与 0334 修过的「只有回填无增量」互为镜像）。
-- ③ ghost 画像存量折叠：note_ghost 曾把变化的自报读数拼进 reason，
--    ON CONFLICT 恒不命中 → 同 (user, agent) 下 hits=1 的行可无限膨胀。
--    按 (user_id, agent) 折叠为单行（hits 求和），reason 统一为固定文案
--    （与 worker 侧新常量逐字一致）。
-- ④ 0311 注释声称“绝对阈值 cheat_gap_threshold_gb 已存在”不属实——
--    该键与 cheat_auto_warn 从未有种子行/settings_meta，站长后台不可调。

BEGIN;

-- ============ ① season 翻转修复 ============
-- anime/movie/documentary 语义的 season 恢复 select（词表 0320 已种入）
UPDATE section_kinds SET field_type = 'select'
WHERE kind = 'season' AND field_type <> 'select'
  AND NOT EXISTS (SELECT 1 FROM torrent_sections ts WHERE ts.kind = 'season'
                    AND ts.value IS NOT NULL
                    AND jsonb_typeof(ts.value) = 'string'
                    AND ts.value #>> '{}' !~ '^(第.+季|Season [0-9]+|S[0-9]+)$');
-- sports 的自由赛季改走独立 kind（无种子引用，纯声明位）
INSERT INTO section_kinds (kind, label, field_type, sort)
VALUES ('season_label', '赛季', 'text', 35)
ON CONFLICT (kind) DO UPDATE SET field_type = 'text';
UPDATE site_type_packs
SET sections = jsonb_build_object(
      'kinds',
        (SELECT jsonb_agg(
           CASE WHEN kd->>'kind' = 'season'
                THEN kd || '{"kind":"season_label"}'::jsonb
                ELSE kd END)
         FROM jsonb_array_elements(sections->'kinds') AS kd),
      'dict',
        (SELECT jsonb_object_agg(key, value) || jsonb_build_object(
            'season_label', sections->'dict'->'season')
         FROM jsonb_each(sections->'dict') WHERE key <> 'season'))
  || (sections - 'kinds' - 'dict')
WHERE code = 'sports' AND jsonb_typeof(sections) = 'object'
  AND sections->'kinds' IS NOT NULL
  AND EXISTS (SELECT 1 FROM jsonb_array_elements(sections->'kinds') kd
               WHERE kd->>'kind' = 'season');

-- ============ ② 0338 锚点补回填（与 0327 同构，扩 kind） ============
INSERT INTO artists (kind, name, norm_name)
SELECT DISTINCT k.kind, TRIM(n.name),
       LOWER(REGEXP_REPLACE(TRIM(n.name), '\s+', '', 'g'))
FROM (
  SELECT unnest(ARRAY['author', 'studio']) AS kind
) k
CROSS JOIN LATERAL (
  SELECT jsonb_array_elements_text(
           CASE jsonb_typeof(ts.value)
             WHEN 'array' THEN ts.value
             ELSE jsonb_build_array(ts.value #>> '{}')
           END) AS name
  FROM torrent_sections ts
  WHERE ts.kind = k.kind
) n
WHERE TRIM(n.name) <> ''
ON CONFLICT (kind, name) DO NOTHING;

-- ============ ③ ghost 存量折叠 ============
-- 旧 reason 内嵌“（自报下载 N 字节…）/（声称上传 N 字节）”变化读数；
-- 折叠成与 worker 新常量逐字一致的固定文案，hits 求和保留画像权重。
INSERT INTO cheat_events (user_id, agent, peer_ip, reason, hits, first_seen, last_seen)
SELECT user_id, agent, MIN(peer_ip),
       'ghost_seed（声称数据完整但从未下载，疑似幽灵做种；跨种/二传用户可申诉）',
       SUM(hits), MIN(first_seen), MAX(last_seen)
FROM cheat_events
WHERE agent LIKE 'ghost:%'
  AND reason LIKE 'ghost_seed%（自报下载%'
GROUP BY user_id, agent
HAVING SUM(hits) > 0
ON CONFLICT (user_id, agent, reason) DO UPDATE
  SET hits = cheat_events.hits + EXCLUDED.hits,
      last_seen = GREATEST(cheat_events.last_seen, EXCLUDED.last_seen);
DELETE FROM cheat_events
WHERE agent LIKE 'ghost:%'
  AND reason LIKE 'ghost_seed%（自报下载%';
-- “声称上传”变体同样折叠（同一缺陷的另一半）
INSERT INTO cheat_events (user_id, agent, peer_ip, reason, hits, first_seen, last_seen)
SELECT user_id, agent, MIN(peer_ip),
       'ghost_seed（声称数据完整但从未下载，疑似幽灵做种；跨种/二传用户可申诉）',
       SUM(hits), MIN(first_seen), MAX(last_seen)
FROM cheat_events
WHERE agent LIKE 'ghost:%'
  AND reason LIKE 'ghost_seed%（声称上传%'
GROUP BY user_id, agent
HAVING SUM(hits) > 0
ON CONFLICT (user_id, agent, reason) DO UPDATE
  SET hits = cheat_events.hits + EXCLUDED.hits,
      last_seen = GREATEST(cheat_events.last_seen, EXCLUDED.last_seen);
DELETE FROM cheat_events
WHERE agent LIKE 'ghost:%'
  AND reason LIKE 'ghost_seed%（声称上传%';
-- 兜底：其余历史变体 reason（含「声称上传 0 字节」等）合并后清行
INSERT INTO cheat_events (user_id, agent, peer_ip, reason, hits, first_seen, last_seen)
SELECT user_id, agent, MIN(peer_ip),
       'ghost_seed（声称数据完整但从未下载，疑似幽灵做种；跨种/二传用户可申诉）',
       SUM(hits), MIN(first_seen), MAX(last_seen)
FROM cheat_events
WHERE agent LIKE 'ghost:%'
  AND reason <> 'ghost_seed（声称数据完整但从未下载，疑似幽灵做种；跨种/二传用户可申诉）'
GROUP BY user_id, agent
  HAVING SUM(hits) > 0
ON CONFLICT (user_id, agent, reason) DO UPDATE
  SET hits = cheat_events.hits + EXCLUDED.hits,
      last_seen = GREATEST(cheat_events.last_seen, EXCLUDED.last_seen);
DELETE FROM cheat_events
WHERE agent LIKE 'ghost:%'
  AND reason <> 'ghost_seed（声称数据完整但从未下载，疑似幽灵做种；跨种/二传用户可申诉）';

-- ============ ④ 0311 两键补入库 ============
INSERT INTO site_settings (name, value, descr, grp)
VALUES
  ('cheat_gap_threshold_gb', '50',
   '流量差额审计的绝对阈值（GiB）：窗口内单种上传-下载净差超过该值才立案，'
   '与相对比率 cheat_gap_ratio 取 AND。出厂默认 50 是公开值。',
   'anticheat'),
  ('cheat_auto_warn', 'yes',
   '作弊事件累进处置的 L1 自动告知开关：yes=达 L1 线给用户发站内提醒，'
   'no=只进管理组台账不发信。',
   'anticheat')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, group_key, card_order, visible,
   min, max, step)
VALUES
  ('cheat_gap_threshold_gb', 'number', '流量差额绝对线', 'Traffic gap absolute',
   '7 天窗口内某种子 SUM(up)-SUM(down) 净差超过该 GiB 数（且满足比率线）'
   '才立案审计。两线同时满足，防小流量误报。',
   'GiB', 'anticheat', 24, true, 1, 100000, 1),
  ('cheat_auto_warn', 'enum', '作弊 L1 自动告知', 'Cheat auto warn',
   'yes=作弊事件达 L1 线自动给用户发提醒信；no=只留管理组台账。',
   '', 'anticheat', 25, true, NULL, NULL, NULL)
ON CONFLICT (name) DO NOTHING;

-- enum 选项（settings_meta.options JSON，0275 口径）
UPDATE settings_meta
SET options = '{"options": [{"label": "开（发提醒信）", "value": "yes"}, {"label": "关（仅台账）", "value": "no"}]}'::jsonb
WHERE name = 'cheat_auto_warn' AND options IS NULL;

COMMIT;
