-- 0128 娱乐域文案订正（0127 机械替换后的残留重复，产品决策 2026-09-19「站型无关」）
--
-- 0127 用 replace('好学农场','农场') 清掉了旧站名，但产生了「农场（农场种植/…）」这类重复读法。
-- 本迁移按最终文案精确赋值（幂等，可重复执行）。

-- 后台「站点设定」里的模块开关标签
UPDATE settings_meta SET label_zh = '农场（种植 / 浇水 / 收获与市场价窗口）'
 WHERE name = 'module_farm';
UPDATE settings_meta SET label_zh = '娱乐屋（刮刮乐 / 猜大小 / 九宫格 / 趣味投票）'
 WHERE name = 'module_games';
UPDATE settings_meta SET label_zh = '五子棋（联机对局）'
 WHERE name = 'module_gomoku';

-- 模块注册表描述
UPDATE modules SET descr = '种植 / 浇水 / 收获与市场价窗口'
 WHERE key = 'farm';
UPDATE modules SET descr = '刮刮乐 / 猜大小 / 九宫格 / 趣味投票（机会类玩法，合规敏感）'
 WHERE key = 'games';
