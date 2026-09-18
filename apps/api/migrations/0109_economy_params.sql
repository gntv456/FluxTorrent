-- 0109: 通用建站系统 U2 —— 经济数值参数化（策划案 §12.2）
--
-- games.rs 编译期常量 → site_settings 键（缺省回落代码默认值，T3 存量站零感知）。
-- 仅迁移「代码常量类」；已是设置键（seeding_base_hourly 等）与迁移种子（商店定价）不动。
-- settings_meta 同步登记「经济参数」分组，后台可调；站型包 economy 预设覆盖同一批键。

INSERT INTO site_settings (name, value, descr, grp) VALUES
-- 娱乐风控（games.rs）
('games_max_bet', '1000', '单次游戏下注上限（火花）', 'module_fun'),
('games_max_plays_per_hour', '60', '每用户每小时游戏次数上限', 'module_fun'),
('games_scratch_empty_pct', '45', '刮刮乐空奖概率 %', 'module_fun'),
('games_scratch_half_pct', '30', '刮刮乐保底 0.5x 概率 %', 'module_fun'),
('games_scratch_one_pct', '15', '刮刮乐 1x 概率 %', 'module_fun'),
('games_scratch_two_pct', '8', '刮刮乐 2x 概率 %', 'module_fun'),
('games_bigsmall_win_mult', '2', '猜大小猜中赔率倍数', 'module_fun'),
-- 签到（M12 口径）
('attendance_first', '10', '签到首签奖励', 'economy'),
('attendance_streak', '5', '连签每日奖励', 'economy'),
('attendance_daily_cap', '1000', '签到单日奖励封顶', 'economy'),
-- 银行利率上限提示（实际五档利率在迁移种子利率表，此处为展示上限）
('bank_max_rate_pct', '18', '银行最高年化利率 %（展示口径）', 'economy'),
-- 农场
('farm_market_window_hours', '4', '农场市场价刷新窗口（小时）', 'module_fun'),
('farm_water_spark', '1', '农场浇水消耗火花', 'module_fun'),
-- 站免池
('magic_pool_target_default', '100000', '站免池默认目标值（火花）', 'economy')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, unit, min, max, group_key, card_order) VALUES
('games_max_bet', 'number', '游戏单注上限', 'Max bet', '火花', 1, 100000, 'module_fun', 10),
('games_max_plays_per_hour', 'number', '游戏每小时限次', 'Plays per hour', '次', 1, 1000, 'module_fun', 11),
('games_scratch_empty_pct', 'number', '刮刮乐空奖概率', 'Scratch empty', '%', 0, 100, 'module_fun', 12),
('games_scratch_half_pct', 'number', '刮刮乐保底概率', 'Scratch half', '%', 0, 100, 'module_fun', 13),
('games_scratch_one_pct', 'number', '刮刮乐 1x 概率', 'Scratch 1x', '%', 0, 100, 'module_fun', 14),
('games_scratch_two_pct', 'number', '刮刮乐 2x 概率', 'Scratch 2x', '%', 0, 100, 'module_fun', 15),
('games_bigsmall_win_mult', 'number', '猜大小赔率', 'Bigsmall multiplier', 'x', 1, 10, 'module_fun', 16),
('attendance_first', 'number', '签到首签奖励', 'First check-in', '火花', 0, 1000, 'economy', 20),
('attendance_streak', 'number', '连签奖励', 'Streak bonus', '火花', 0, 1000, 'economy', 21),
('attendance_daily_cap', 'number', '签到单日封顶', 'Daily cap', '火花', 1, 100000, 'economy', 22),
('bank_max_rate_pct', 'number', '银行利率上限', 'Bank max rate', '%', 1, 100, 'economy', 23),
('farm_market_window_hours', 'number', '农场市场窗口', 'Farm market window', '小时', 1, 48, 'module_fun', 17),
('farm_water_spark', 'number', '浇水消耗', 'Water cost', '火花', 0, 100, 'module_fun', 18),
('magic_pool_target_default', 'number', '站免池目标', 'Pool target', '火花', 1000, 100000000, 'economy', 24)
ON CONFLICT (name) DO UPDATE
  SET label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en, unit = EXCLUDED.unit,
      min = EXCLUDED.min, max = EXCLUDED.max, group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;
