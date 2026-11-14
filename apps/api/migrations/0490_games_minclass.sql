-- 0490: 娱乐屋逐游戏等级门槛（站长可调：九款游戏各一个 classlevel 键）。
--
-- 背景：九款玩法此前只有登录闸（require_auth），0 级新号也能开玩——
-- 模块开关（module_games/gacha/farm）是整域粒度，到不了「锁扭蛋留刮刮乐」。
-- 对齐 NP 的 classlevel 先例（authority_* 53 键 / 论坛版块 minclassread）：
-- 每款游戏一个 classlevel 设置键，缺省 0 = 不限等级。
--
-- 消费：games_http/helpers.rs 的 check_minclass（API 硬闸）+ /games 总览
-- 下发 gates 清单（前端大厅卡片置灰标「N 级解锁」）。
--
-- 幂等：settings_meta / site_settings 双 ON CONFLICT DO NOTHING，重跑安全。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('games_minclass_bigsmall', '0', '猜大小最低等级', 'games'),
('games_minclass_scratch',  '0', '刮刮乐最低等级', 'games'),
('games_minclass_jgg',      '0', '九宫格最低等级', 'games'),
('games_minclass_capsule',  '0', '扭蛋最低等级', 'games'),
('games_minclass_wheel',    '0', '大转盘最低等级', 'games'),
('games_minclass_farm',     '0', '农场最低等级', 'games'),
('games_minclass_gacha',    '0', '抽卡最低等级', 'games'),
('games_minclass_pet',      '0', '宠物最低等级', 'games'),
('games_minclass_fishing',  '0', '钓鱼最低等级', 'games')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint,
                           group_key, card_order) VALUES
('games_minclass_bigsmall', 'classlevel', '猜大小等级门槛',
 'Big-small min class',
 '达到该等级才能玩；0 = 不限。低于门槛的用户看不到入口，直接调 API 也会被拒',
 '娱乐屋', 1),
('games_minclass_scratch', 'classlevel', '刮刮乐等级门槛',
 'Scratch min class',
 '达到该等级才能玩；0 = 不限。低于门槛的用户看不到入口，直接调 API 也会被拒',
 '娱乐屋', 2),
('games_minclass_jgg', 'classlevel', '九宫格等级门槛',
 'Nine-grid min class',
 '达到该等级才能玩；0 = 不限。低于门槛的用户看不到入口，直接调 API 也会被拒',
 '娱乐屋', 3),
('games_minclass_capsule', 'classlevel', '扭蛋等级门槛',
 'Capsule min class',
 '达到该等级才能玩；0 = 不限。低于门槛的用户看不到入口，直接调 API 也会被拒',
 '娱乐屋', 4),
('games_minclass_wheel', 'classlevel', '大转盘等级门槛',
 'Wheel min class',
 '达到该等级才能玩；0 = 不限。低于门槛的用户看不到入口，直接调 API 也会被拒',
 '娱乐屋', 5),
('games_minclass_farm', 'classlevel', '农场等级门槛',
 'Farm min class',
 '达到该等级才能玩；0 = 不限。低于门槛的用户看不到入口，直接调 API 也会被拒',
 '娱乐屋', 6),
('games_minclass_gacha', 'classlevel', '抽卡等级门槛',
 'Gacha min class',
 '达到该等级才能玩；0 = 不限。低于门槛的用户看不到入口，直接调 API 也会被拒',
 '娱乐屋', 7),
('games_minclass_pet', 'classlevel', '宠物等级门槛',
 'Pet min class',
 '达到该等级才能玩；0 = 不限。低于门槛的用户看不到入口，直接调 API 也会被拒',
 '娱乐屋', 8),
('games_minclass_fishing', 'classlevel', '钓鱼等级门槛',
 'Fishing min class',
 '达到该等级才能玩；0 = 不限。低于门槛的用户看不到入口，直接调 API 也会被拒',
 '娱乐屋', 9)
ON CONFLICT (name) DO NOTHING;
