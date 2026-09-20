-- 0131 娱乐玩法参数补齐（产品决策 2026-09-19 续：遗留三处配置化）
--
-- 1) games_scratch_ten_pct —— 刮刮乐 10x 档概率。0109 只登记了前四档，10x 一直是「余数」，
--    站长无法单独调头奖概率。补上后五档全可配（代码侧 10x = 100 − 其余四档和，校验越界回落缺省）。
-- 2) farm_max_plays_per_hour —— 农场自己的每小时配额（代码已在读，但 settings_meta 缺行 →
--    后台站点设定页看不到、改不了，实际恒为代码缺省 30）。
-- 3) farm_wither_days —— 农场作物有效期（枯萎天数）。旧口径「农作物 5 天有效期」此前只有
--    迁移注释、代码没实现。现补参数：>0 表示过熟 N 天后枯萎（收获作废、地块清空），
--    0 表示永不枯萎（站长可关）。
--
-- 幂等：ON CONFLICT DO NOTHING / DO UPDATE，可重复执行。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('games_scratch_ten_pct', '2', '刮刮乐 10x 概率 %（不填则取余数）', 'module_fun'),
('farm_max_plays_per_hour', '30', '农场每小时操作上限（与赌局分开计数）', 'module_fun'),
('farm_wither_days', '5', '农场作物有效期（天，0 = 永不枯萎）', 'module_fun')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, unit, min, max, step, hint, group_key, card_order) VALUES
('games_scratch_ten_pct', 'number', '刮刮乐 10x 概率', 'Scratch 10x', '%', 0, 100, 1,
 '五档概率合计不得超过 100%；本档留空/为 0 时取余数', 'module_fun', 19),
('farm_max_plays_per_hour', 'number', '农场每小时限次', 'Farm ops per hour', '次', 1, 1000, 1,
 '农场与赌局分开计数：种满 6 块地不会占用赌局的下注额度', 'module_fun', 20),
('farm_wither_days', 'number', '作物有效期', 'Crop lifetime', '天', 0, 60, 1,
 '成熟后超过该天数未收获即枯萎（收获作废并清空地块）；0 = 永不枯萎', 'module_fun', 21)
ON CONFLICT (name) DO UPDATE
  SET label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en, unit = EXCLUDED.unit,
      min = EXCLUDED.min, max = EXCLUDED.max, step = EXCLUDED.step,
      hint = EXCLUDED.hint, group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;
