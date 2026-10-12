-- 0350_drop_attendance_legacy_keys.sql
-- P-97 签到域假开关收口（2026-10-12 批三十三实测）。
--
-- settings_meta / site_settings 里摆着 7 个 attendance_* 键，代码只读
-- attendance_first / attendance_streak / attendance_daily_cap 三个：
--   attendance_continuous_day = "10,20,30"
--   attendance_continuous_value = "200,500,1000"
--     —— 里程碑此前硬编码在 economy::checkin_reward 的 match 里，这两键
--     零消费者（全仓 grep 命中 0）。站长在后台改数字不生效，且界面上看到的
--     数字与真实生效的那份「撞脸」，最容易被当成已生效。
--     → 已在代码侧接线读取（脏值/条数不齐回落内置 10/20/30→200/500/1000）。
--   attendance_initial=10 / attendance_step=5 / attendance_max=1000
--     —— 旧站签到公式的三份残留，与在读的 first/streak/daily_cap 同义，
--     同一功能摆两套数字必然改漏一份（0156 勋章加成同族教训）。
--   attendance_bonus=200 —— 与里程碑首档加送同义的孤立副本。
--   attendance_card=1000 —— 补签卡价格的孤立副本，真值在 shop_items.price
--     （id=12 补签卡，实测 1000）。
-- 按 0193/0348 先例把后 5 个零消费者键从目录与现值里摘除。
DELETE FROM settings_meta WHERE name IN
    ('attendance_initial', 'attendance_step', 'attendance_max',
     'attendance_bonus', 'attendance_card');
DELETE FROM site_settings WHERE name IN
    ('attendance_initial', 'attendance_step', 'attendance_max',
     'attendance_bonus', 'attendance_card');
