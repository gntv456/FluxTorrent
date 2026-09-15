-- 0084: worker 加固批次（审计遗留项）—— 两个新站点设定键。
--
-- 1) hr_violation_limit：H&R 违规下载暂停阈值（默认 3）。
--    worker hr_punish：未解决违规数（hr_violations.resolved_at IS NULL）≥ 阈值
--    → users.download_enabled = false + PM；降回阈值以下自动恢复。
-- 2) preserve_dead_days：死种入保种区天数（默认 7）。
--    worker preserve_seed：seeders=0 AND leechers=0 AND approval_status=1
--    且创建时间早于该天数、不在 seed_preserve 中的种子入保种区（保种区数据源）。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('hr_violation_limit', '3', '未解决 H&R 违规数达到该值即暂停下载权限', 'account'),
('preserve_dead_days', '7', '无种无下载的过审种子入保种区前的存活天数', 'main')
ON CONFLICT (name) DO UPDATE SET descr = EXCLUDED.descr;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, unit, min, max, step, group_key, card_order) VALUES
('hr_violation_limit', 'number', 'H&R 违规下载暂停阈值', 'HR violation limit', '未解决 H&R 违规数达到该值即自动暂停下载权限；降回阈值以下自动恢复（自助免罪/Pardon 可消除违规）', '次', 1, 100, 1, '等级升降', 8),
('preserve_dead_days', 'number', '死种入保种区天数', 'Dead torrent preserve days', '过审种子无做种且无下载达到该天数后自动进入保种区等待认领', '天', 1, 365, 1, '上传限制', 8)
ON CONFLICT (name) DO UPDATE SET
    label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
    hint = EXCLUDED.hint, unit = EXCLUDED.unit,
    min = EXCLUDED.min, max = EXCLUDED.max, step = EXCLUDED.step,
    group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;
