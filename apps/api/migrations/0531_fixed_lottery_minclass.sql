-- 0531 定档开奖等级门槛：games_minclass_lottery（2026-11-28）
--
-- 0530 收口时确认：十二款玩法里唯一没接等级闸的就是定档开奖
-- （fixed_lottery 用户口「权限：登录即可」）。本迁移补齐设置键，
-- 硬闸接线在 fixed_lottery_http 的报名/领取两个写口（读口不锁，
-- 大厅入口可见，报名时才拦）。缺省 0 = 不限——上线即静默生效，
-- 站长在后台娱乐屋分组调成 N 级才开闸。
--
-- 幂等：settings_meta / site_settings 双 ON CONFLICT DO NOTHING。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('games_minclass_lottery', '0', '定档开奖最低等级', 'games')
ON CONFLICT (name) DO NOTHING;

-- min_class 显式写 98：0530 已实证本族 INSERT 不带该列会落建表缺省 99，
-- 使站长号（98）只读（0493 只拉平了当时存量，帮不了后插入的键）。
INSERT INTO settings_meta (name, type, label_zh, label_en, hint,
                           group_key, card_order, min_class) VALUES
('games_minclass_lottery', 'classlevel', '定档开奖等级门槛',
 'Lottery min class',
 '达到该等级才能报名；0 = 不限。低于门槛的用户仍能看到入口，报名时会被拒',
 '娱乐屋', 12, 98)
ON CONFLICT (name) DO NOTHING;

