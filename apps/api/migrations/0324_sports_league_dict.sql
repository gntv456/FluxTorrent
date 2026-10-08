-- 0324_sports_league_dict.sql
-- 站型成熟度 · sports 收尾（§7.5 最后一站型的可落地半场）：
--
-- 0315 给 sports 配了 league/season/round 三维度但 league 没配词表
-- （声明了 kind 没配 dict = 装饰，与 anime subtitle_group 同病）；
-- season 在旧 apply 路径下回落成 select（0315 时代丢 field_type）。
-- 本迁移：league 词表补齐（主流联赛口径）+ season 空引安全升级 text。
--
-- 对阵实体（match/比分）仍属 P1 深水区：league/season/round 维度组合
-- 已支撑「只看英超第 10 轮」检索（闸门实测），对阵页/比分另批评估。

BEGIN;

-- league 词表（足球五大联赛 + 篮球/综合口径）
INSERT INTO section_dict (kind, name, sort)
SELECT 'league', v.n, v.s FROM (VALUES
  ('英超', 10), ('西甲', 20), ('德甲', 30), ('意甲', 40), ('法甲', 50),
  ('中超', 60), ('欧冠', 70), ('NBA', 80), ('CBA', 90), ('其他赛事', 100)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
    WHERE d.kind='league' AND d.name=v.n);

-- sports 包 dict 带上（apply 重建时生效）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{dict,league}',
  '["英超","西甲","德甲","意甲","法甲","中超","欧冠","NBA","CBA","其他赛事"]'::jsonb, true)
WHERE code='sports';

-- season 空引安全升级（0320 同款口径：零引用 + 根因是旧 apply 丢字段）
UPDATE section_kinds SET field_type='text'
WHERE kind='season' AND field_type='select'
  AND NOT EXISTS (SELECT 1 FROM torrent_sections ts WHERE ts.kind='season');

COMMIT;
