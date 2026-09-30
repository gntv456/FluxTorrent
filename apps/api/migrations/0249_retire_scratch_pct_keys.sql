-- 刮刮乐的概率设置键停读之后删掉（值已在 0248 搬进 arcade_pools / arcade_pool_entries）。
--
-- 留着不删 = 设置面板里五个「改了没反应」的假开关：站长在界面上把
-- `games_scratch_half_pct` 从 30 调成 60，什么都不会发生，而奖池编辑器里
-- 同一档的权重还是 30 —— 两份清单同时存在时，用户永远先改到那一份死的。
-- 停读只是第一步，删键才算真的只有一份。
DELETE FROM settings_meta WHERE name LIKE 'games_scratch_%_pct';
DELETE FROM site_settings WHERE name LIKE 'games_scratch_%_pct';

DO $$
DECLARE n_left bigint;
BEGIN
    SELECT count(*) INTO n_left FROM site_settings
     WHERE name LIKE 'games_scratch_%_pct';
    IF n_left > 0 THEN
        RAISE EXCEPTION '% 个刮刮乐概率键没删干净：设置面板会留下假开关', n_left;
    END IF;
END $$;
