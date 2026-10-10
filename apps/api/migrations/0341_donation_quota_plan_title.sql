-- 0341 捐赠 quota 档文案纠偏
-- plan_type='quota' 发放的是 users.quota_extra（领邀请码时优先消耗的额外名额），
-- 旧标题「10 个片单额度」里的「片单」在本站没有任何对应实体——馒头 donate 口径
-- 直译残留，用户在捐赠页买了、在邀请页只看到「额外配额」，两头对不上。
-- 标题首段数字即发放量（见 staff_http/donate_notify.rs），改文案必须保持
-- 「<数字> <单位> <名称>」的空格分隔形状。
UPDATE donation_plans SET title = '10 枚邀请名额'
WHERE id = 1 AND plan_type = 'quota' AND title = '10 个片单额度';
