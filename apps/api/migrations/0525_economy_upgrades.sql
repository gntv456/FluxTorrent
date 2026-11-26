-- 0525: 经济系统七件套（对照 StageAbove 立项批）——
--   等级做种加成 + 官方种区域加成 + 流量积分基线表 + 临时邀请 7 天档 SKU。
--
-- 背景（2026-11-26）：等级只影响升级门槛与 H&R 分档，不影响收益，
-- 升级缺获得感；官种有 0488 单种倍率但缺「区域加成」第二层口径；
-- 上传/下载量完全不产生收益。本迁移补三类配置与真相源函数，
-- worker 结算（seeding.rs / traffic_reward.rs）与 API 预估
-- （seeding_est.rs / spark_tiers.rs / my_spark.rs）必须同源调用。
--
-- 不改 seeding_params() 的 RETURNS TABLE：PostgreSQL 的
-- CREATE OR REPLACE FUNCTION 不允许变更返回列（0488 头注释同坑），
-- 故新增独立函数做真相源。
--
-- 转载区加成（area_reprint_rate）刻意缺位：repost 功能整体未实现
-- （0062），无标记可挂，等实装时同步补。
--
-- 幂等：ON CONFLICT / CREATE OR REPLACE / IF NOT EXISTS。

BEGIN;

---------------------------------------------------------------
-- ① 等级做种加成：class 2..12 各 2%..22%，贵宾 20 固定 10%
---------------------------------------------------------------
INSERT INTO site_settings (name, value, descr, grp) VALUES
('seeding_class_rate_2',  '0.02', '等级做种加成：幼苗 +2%',  'economy'),
('seeding_class_rate_3',  '0.04', '等级做种加成：小苗 +4%',  'economy'),
('seeding_class_rate_4',  '0.06', '等级做种加成：小树 +6%',  'economy'),
('seeding_class_rate_5',  '0.08', '等级做种加成：幼树 +8%',  'economy'),
('seeding_class_rate_6',  '0.10', '等级做种加成：乔木 +10%', 'economy'),
('seeding_class_rate_7',  '0.12', '等级做种加成：大树 +12%', 'economy'),
('seeding_class_rate_8',  '0.14', '等级做种加成：花蕾 +14%', 'economy'),
('seeding_class_rate_9',  '0.16', '等级做种加成：开花 +16%', 'economy'),
('seeding_class_rate_10', '0.18', '等级做种加成：结果 +18%', 'economy'),
('seeding_class_rate_11', '0.20', '等级做种加成：硕果 +20%', 'economy'),
('seeding_class_rate_12', '0.22', '等级做种加成：森林 +22%', 'economy'),
('seeding_class_rate_20', '0.10', '等级做种加成：贵宾 +10%', 'economy')
ON CONFLICT (name) DO NOTHING;

---------------------------------------------------------------
-- ② 官方种区域加成（第二层口径，与 0488 单种倍率并存）
---------------------------------------------------------------
INSERT INTO site_settings (name, value, descr, grp) VALUES
('seeding_area_official_rate', '0.10',
 '官方种区域加成：官种做种积分额外 +10%（0 = 关闭）', 'economy'),
('seeding_traffic_up_rate', '0.5',
 '流量积分：每上传 1 GB 计 0.5 积分（0 = 关闭）', 'economy'),
('seeding_traffic_down_rate', '0.1',
 '流量积分：每下载 1 GB 计 0.1 积分（0 = 关闭）', 'economy')
ON CONFLICT (name) DO NOTHING;

---------------------------------------------------------------
-- settings_meta 登记：后台「经济参数」面板自动出现（card_order 48 起，
-- 47 已被 0488 seeding_official_mult 占用）
---------------------------------------------------------------
INSERT INTO settings_meta
 (name, type, label_zh, label_en, unit, min, max, step, hint,
  group_key, card_order)
VALUES
 ('seeding_class_rate_2',  'number', '等级加成·幼苗', 'Class rate 2',  '%', 0, 0.5, 0.01, 'class=2 幼苗的做种收益加成百分比', 'economy', 48),
 ('seeding_class_rate_3',  'number', '等级加成·小苗', 'Class rate 3',  '%', 0, 0.5, 0.01, 'class=3 小苗的做种收益加成百分比', 'economy', 49),
 ('seeding_class_rate_4',  'number', '等级加成·小树', 'Class rate 4',  '%', 0, 0.5, 0.01, 'class=4 小树的做种收益加成百分比', 'economy', 50),
 ('seeding_class_rate_5',  'number', '等级加成·幼树', 'Class rate 5',  '%', 0, 0.5, 0.01, 'class=5 幼树的做种收益加成百分比', 'economy', 51),
 ('seeding_class_rate_6',  'number', '等级加成·乔木', 'Class rate 6',  '%', 0, 0.5, 0.01, 'class=6 乔木的做种收益加成百分比', 'economy', 52),
 ('seeding_class_rate_7',  'number', '等级加成·大树', 'Class rate 7',  '%', 0, 0.5, 0.01, 'class=7 大树的做种收益加成百分比', 'economy', 53),
 ('seeding_class_rate_8',  'number', '等级加成·花蕾', 'Class rate 8',  '%', 0, 0.5, 0.01, 'class=8 花蕾的做种收益加成百分比', 'economy', 54),
 ('seeding_class_rate_9',  'number', '等级加成·开花', 'Class rate 9',  '%', 0, 0.5, 0.01, 'class=9 开花的做种收益加成百分比', 'economy', 55),
 ('seeding_class_rate_10', 'number', '等级加成·结果', 'Class rate 10', '%', 0, 0.5, 0.01, 'class=10 结果的做种收益加成百分比', 'economy', 56),
 ('seeding_class_rate_11', 'number', '等级加成·硕果', 'Class rate 11', '%', 0, 0.5, 0.01, 'class=11 硕果的做种收益加成百分比', 'economy', 57),
 ('seeding_class_rate_12', 'number', '等级加成·森林', 'Class rate 12', '%', 0, 0.5, 0.01, 'class=12 森林的做种收益加成百分比', 'economy', 58),
 ('seeding_class_rate_20', 'number', '等级加成·贵宾', 'Class rate VIP', '%', 0, 0.5, 0.01, 'class=20 贵宾的做种收益加成百分比', 'economy', 59),
 ('seeding_area_official_rate', 'number', '官区加成', 'Official area bonus', '%', 0, 1, 0.01, '官方种（official_tag）做种积分额外加成比例；与官种倍率并存', 'economy', 60),
 ('seeding_traffic_up_rate',   'number', '流量积分·上传', 'Traffic up rate',   '积分/GB', 0, 10, 0.1, '每上传 1 GB 的流量积分（独立流水条目，非做种乘数）', 'economy', 61),
 ('seeding_traffic_down_rate', 'number', '流量积分·下载', 'Traffic down rate', '积分/GB', 0, 10, 0.1, '每下载 1 GB 的流量积分（独立流水条目，非做种乘数）', 'economy', 62)
ON CONFLICT (name) DO UPDATE
  SET label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
      unit = EXCLUDED.unit, min = EXCLUDED.min, max = EXCLUDED.max,
      step = EXCLUDED.step, hint = EXCLUDED.hint,
      group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;

---------------------------------------------------------------
-- ③ 真相源函数（STABLE；调用侧务必 MATERIALIZED，0134 nestloop 教训）
---------------------------------------------------------------
-- 等级加成：无配置的等级恒 0（90+ 员工档不设加成是刻意口径）
CREATE OR REPLACE FUNCTION seeding_class_rate(p_class_id int)
RETURNS double precision LANGUAGE sql STABLE AS $$
    SELECT LEAST(0.5::double precision, GREATEST(0.0::double precision,
                 COALESCE(
                     (SELECT value FROM site_settings
                       WHERE name = 'seeding_class_rate_' || p_class_id)::double precision,
                     0.0::double precision)))
$$;

COMMENT ON FUNCTION seeding_class_rate(int) IS
  '等级做种加成比率（worker 结算 + API 预估同源）。'
  ' 读 site_settings.seeding_class_rate_{class_id}，未配置恒 0，钳制 [0,0.5]。'
  ' 乘在 seeding_hourly 结果上；调用侧须 MATERIALIZED。';

-- 官区加成：官方种单种积分额外 +(rate)
CREATE OR REPLACE FUNCTION seeding_area_official_rate()
RETURNS double precision LANGUAGE sql STABLE AS $$
    SELECT LEAST(1.0::double precision, GREATEST(0.0::double precision,
                 COALESCE(
                     (SELECT value FROM site_settings
                       WHERE name = 'seeding_area_official_rate')::double precision,
                     0.10::double precision)))
$$;

COMMENT ON FUNCTION seeding_area_official_rate() IS
  '官方种区域加成比率（worker 结算 + API 预估同源）。'
  ' 读 site_settings.seeding_area_official_rate，缺省 0.10，钳制 [0,1]。'
  ' 官种单种积分乘 (1 + rate)；调用侧须 MATERIALIZED。';

---------------------------------------------------------------
-- ④ 流量积分结算基线：每用户一行，记录上次结算时已计入的
--    uploaded/downloaded 快照；delta = 当前值 - 基线（GREATEST 钳 0，
--    反爬/修正导致的倒退不会产生负流水）
---------------------------------------------------------------
CREATE TABLE IF NOT EXISTS traffic_settle_state (
  user_id BIGINT PRIMARY KEY REFERENCES users(id),
  last_uploaded BIGINT NOT NULL DEFAULT 0,
  last_downloaded BIGINT NOT NULL DEFAULT 0,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

---------------------------------------------------------------
-- ⑤ 临时邀请 7 天档 SKU：零代码生效——flux-shared 发券链已读
--    config.days（0460 引入，clamp 1..=365）。核销后邀请码有效期
--    仍走全局 invite_admin_ttl_days，与 30 天档一致。
--    同 kind 多 SKU 天然共存（card_kind 按判断不受影响）。
---------------------------------------------------------------
INSERT INTO shop_items (name, kind, price, config, active) VALUES
('临时邀请名额(7天)', 'temp_invite', 3000,
 '{"days": 7, "stackable": true}'::jsonb, true)
ON CONFLICT DO NOTHING;

COMMIT;
