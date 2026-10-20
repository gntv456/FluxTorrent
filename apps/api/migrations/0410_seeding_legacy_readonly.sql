-- 0410：把「做种公式」屏上八把无人读取的旧键标成只读（P-460）
--
-- 现状（现网实测）：后台「站点设定 → 做种公式」这张屏里，
--   perseeding(每个做种基础魔力=1) / maxseeding(计费做种上限=7) /
--   bonus_per_seed(做种基础魔力（旧键）=0.4) / donortimes(捐赠者魔力倍数=2) /
--   tzero(公式 T0=4) / nzero(公式 N0=7) / bzero(公式 B0=100) / l(公式 L=300)
-- 八把键**可写可回显，但没有任何读者**：
--   * Rust 侧：剥掉注释后 `apps/`、`crates/` 里搜不到这些键名；
--   * SQL 侧：真正在算做种火花的是数据库函数 `seeding_params()`，它读的是
--     `seeding_base_hourly / seeding_cap_hourly / seeding_bonus_scale /
--      seeding_donor_mult / seeding_curve_k / seeding_rarity_k /
--      seeding_rarity_exp / seeding_vol_log_base_gb` 这八把 `seeding_*` 键，
--     旧键一次都不出现（pg_proc.prosrc 实测）。
--
-- 比「改了没用」更糟的是两套数互相矛盾：面板写着底薪 1、上限 7，
-- 实际发的是 seeding_base_hourly=10、seeding_cap_hourly=150 算出来的。
-- 站长按旧键推算做种经济，推的是一个已经不存在的公式（0136 换公式留下的尾巴）。
--
-- 处置沿用本仓既有口径，不发明新机制：`settings_meta.readonly = true` + hint
-- 说清「谁真正决定」。先例是登录防线那五条兼容保留键（maxloginattempts /
-- maxip / securelogin / securetracker / loginattemptwhitelist）。readonly 在
-- 后端会被逐字段拒绝（"该字段为只读"，settings_http/groups.rs），前端显示
-- 只读徽标并把控件锁住（setting-field-controls.tsx）——值仍然看得见，便于
-- 老运维对照，但改不动了。
--
-- 不删键、不改 site_settings 的值：删行会让站方历史对照断档，而真键的
-- 面板项已经齐全，不需要这里再补。

UPDATE settings_meta
   SET readonly = true,
       hint = '兼容保留键：做种底薪由「做种积分底薪（每小时）」seeding_base_hourly 决定，此项不再参与实际逻辑'
 WHERE name = 'perseeding';

UPDATE settings_meta
   SET readonly = true,
       hint = '兼容保留键：做种封顶由「做种封顶」seeding_cap_hourly 决定，此项不再参与实际逻辑'
 WHERE name = 'maxseeding';

UPDATE settings_meta
   SET readonly = true,
       hint = '兼容保留键：做种标定由「做种总标定」seeding_bonus_scale 决定，此项不再参与实际逻辑'
 WHERE name = 'bonus_per_seed';

UPDATE settings_meta
   SET readonly = true,
       hint = '兼容保留键：捐赠者倍数由「捐赠者倍数」seeding_donor_mult 决定，此项不再参与实际逻辑'
 WHERE name = 'donortimes';

UPDATE settings_meta
   SET readonly = true,
       hint = '兼容保留键：旧 NP 火花公式（T0/N0/B0/L）自 0136 换公式后停用，现行参数是 seeding_curve_k / seeding_rarity_k / seeding_rarity_exp / seeding_vol_log_base_gb'
 WHERE name IN ('tzero', 'nzero', 'bzero', 'l');
