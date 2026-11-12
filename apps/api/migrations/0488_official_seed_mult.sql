-- 0488: 官种做种加成倍率（单种乘数，非独立 A 值叠加）
--
-- 背景（2026-11-12）：官种（torrents.official_tag）此前对做种收益零加成；
-- 且官种通常做种者多，稀有度因子 (1 + k * seeders^-exp) 趋近 1，
-- 同样体积/种龄下保热门官种反而比保独苗普通种拿得少。
--
-- 不照抄 NP 的「官种单独算一份 A × 系数」：NP 是四份 A 各自 arctan 后
-- 相加，天然绕过封顶；我们是单一 Σ → 一次 arctan 封顶（0136 标定
-- scale=0.4 专门堵「堆小种」套利）。往 Σ 里塞独立 ×6 会把标定作废。
--
-- 做法：沿用既有「单种乘数」链（盒子 bonus_factor / 发布者衰减），按
-- official_tag 给单种加成乘一个数，仍进 Σ → arctan 压缩，封顶不破。
-- worker 结算（jobs/seeding.rs）与 API 预估（voucher_use.rs my_spark）
-- 必须同源调用本函数，否则用户看到的预估与实际到账对不上。
--
-- 不改 seeding_params() 的 RETURNS TABLE：PostgreSQL 的
-- CREATE OR REPLACE FUNCTION 不允许变更返回列，故用独立函数做真相源。
--
-- 幂等：ON CONFLICT / CREATE OR REPLACE。

BEGIN;

INSERT INTO site_settings (name, value, descr, grp) VALUES
('seeding_official_mult', '1.5',
 '官种做种收益倍率（单种加成乘数，1.0 = 关闭）', 'economy')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
 (name, type, label_zh, label_en, unit, min, max, step, hint,
  group_key, card_order)
VALUES
 ('seeding_official_mult', 'number', '官种倍率',
  'Official torrent multiplier', 'x', 1, 3, 0.05,
  '官种种子的单种加成乘此倍率；1.0 = 不加成。仍进 arctan 封顶，不破标定',
  'economy', 47)
ON CONFLICT (name) DO UPDATE
  SET label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
      unit = EXCLUDED.unit, min = EXCLUDED.min, max = EXCLUDED.max,
      step = EXCLUDED.step, hint = EXCLUDED.hint,
      group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;

-- STABLE：调用侧务必放进 MATERIALIZED CTE（见 0134 的 nestloop 教训），
-- 否则优化器会内联进每行，逐颗种子读一次 site_settings。
-- clamp 到 [1,3]：0 会让官种收益归零，过大则击穿 0136 标定。
CREATE OR REPLACE FUNCTION seeding_official_mult()
RETURNS double precision LANGUAGE sql STABLE AS $$
    SELECT LEAST(3.0::double precision,
                 GREATEST(1.0::double precision, COALESCE(
                     (SELECT value FROM site_settings
                       WHERE name = 'seeding_official_mult')::double precision,
                     1.5::double precision)))
$$;

-- PostgreSQL 的 COMMENT 只接受字符串字面量（不能用 || 运算），
-- 换行分隔的相邻字面量由 SQL 标准自动拼接。
COMMENT ON FUNCTION seeding_official_mult() IS
  '官种做种加成倍率（worker 结算 + API 预估同源）。'
  ' 读 site_settings.seeding_official_mult，缺省 1.5，钳制 [1,3]。'
  ' 只乘单种加成，仍进 Σ → arctan 封顶；调用侧须 MATERIALIZED。';

COMMIT;
