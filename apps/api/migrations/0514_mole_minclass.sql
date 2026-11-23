-- 0514: 打地鼠等级门槛键补登（0490 九键漏了 mole——游戏本体和总览
-- 下发都用了 games_minclass_mole，但键从未入 settings_meta/site_settings，
-- 闸读数恒 0 放行。幂等补登，与 0490 同构：双 ON CONFLICT DO NOTHING。
--
-- 甜梦钓场（0513，/games/fishing/pond）不设独立键：它就是 fishing 玩法的
-- 完整版，共用 games_minclass_fishing（API 硬闸已在其 6 个端点接线）。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('games_minclass_mole', '0', '打地鼠最低等级', 'games')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint,
                           group_key, card_order) VALUES
('games_minclass_mole', 'classlevel', '打地鼠等级门槛',
 'Mole min class',
 '达到该等级才能玩；0 = 不限。低于门槛的用户看不到入口，直接调 API 也会被拒',
 '娱乐屋', 10)
ON CONFLICT (name) DO NOTHING;
