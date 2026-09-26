-- 0219：魔力经济参数校准（评审 P1-8 + 收支平衡表结论）
--
-- 现状：贷款额度常数 bank_loan_ratio_constant=1000（0048 迁移），即单笔上限
--   = 时魔×100 + 1000 —— 做种冷启动用户上限仅 ~1000 魔力，而商店补签卡
--   5000 / 改名卡 10000，贷款功能形同虚设。
-- 处理：
--   1) 常数 1000 → 50000，系数 100 → 200：让贷款真正能覆盖「应急购物」场景
--      （老用户时魔 ≈ 300/小时 → 上限 11 万；纯新人 ≈ 5 万，够买 10 张补签卡）。
--      利率不变（日息 0.08%~0.22%），仍远高于存款利率，套利空间不变。
--   2) 签到奖励 10 → 15：当前 1GB 上传量 3000 魔力，签到 500 天才换得起，
--      与「签到得魔力」的引导强度不匹配（详见 _doc/魔力经济收支平衡表-2026-09-26.md）。
--      已签出历史流水不改——只影响之后的新签到。

UPDATE site_settings SET value = '200'  WHERE name = 'bank_loan_ratio';
UPDATE site_settings SET value = '50000' WHERE name = 'bank_loan_ratio_constant';

-- 签到奖励基线（attendance_* 是签到奖励的真实键名，economy.rs
--   CheckInParams::from_settings 读这三键；首签/连签 10/5 → 15/8）。
UPDATE site_settings SET value = '15' WHERE name = 'attendance_first';
UPDATE site_settings SET value = '8'  WHERE name = 'attendance_streak';
