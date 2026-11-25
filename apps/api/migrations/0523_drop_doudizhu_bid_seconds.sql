-- 0523：下线 doudizhu_bid_seconds 假开关（fake_switch 门禁抓到的
-- 「后台可写但全仓无人读取」）。斗地主叫分是同步状态机（叫分不设倒计时，
-- 超时托管统一走 doudizhu_turn_seconds 派生的挂机阈值），该键从未接线。
-- 另两个同批键已接线：doudizhu_rake_permille（settle_repo 同事务读）、
-- doudizhu_pvp_idle_minutes（doudizhu_http idle_seconds 优先读）。
-- 0521 已在本地库应用，不能改原文，按惯例追加清理迁移。

DELETE FROM settings_meta WHERE name = 'doudizhu_bid_seconds';
DELETE FROM site_settings WHERE name = 'doudizhu_bid_seconds';
