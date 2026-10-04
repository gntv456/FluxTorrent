-- 0277：默认等级要求对齐 NexusPHP 官方口径
--
-- 背景：本站缺省 12 级「植物系」（新芽→生态）+ uploaded/dl_count/seed_hours/age
-- 四维门槛是 0058 自定的。对标调研（.np-research 官方仓库 php8 分支
-- settings.default.php account 段 + cleanup.php promotion()）结论：
--   · NP 官方晋升级名 = Peasant/User/Power User/.../Nexus Master（十级），
--     中文语言包同样保留英文原名；植物系是各站二开，非官方。
--   · NP 门槛维度 = 下载量 GB + 分享率 + 注册周数 + 做种积分（无 uploaded 门槛）。
--     本站 worker 判定是 uploaded/完成数/做种小时/账龄四维——机制不同，
--     对齐的是**数值梯度与周数口径**，维映射如下：
--       NP downloaded GB  → 本站 min_uploaded（同名维度里最接近的量纲；
--                           本站升级主口径一直是上传量，保持机制不动）
--       NP 注册周数       → min_account_age_days（×7 天）
--       NP seed_points    → 无直接对应（只增计数器 vs 我们 seed_hours）；
--                           min_seed_hours 按等级比例给温和值，不照抄
--       （完成数 min_download_count NP 无此维，给 0 = 不设卡）
--   · NP 降级只看分享率一维；本站 demotable 全真即可（worker 同套四维判降）。
--
-- 已运营站点可自行在后台再改——本迁移只改「出厂默认」。站点类型包
-- （site_pack applies，0223）会在装站向导时按站型覆盖 name，门槛不动。

BEGIN;

-- 1) 等级名：十级成长线（NP 官方原名）+ VIP/职务级不动
UPDATE user_classes SET name = 'Peasant'    WHERE id = 0;
UPDATE user_classes SET name = 'User'       WHERE id = 1;
UPDATE user_classes SET name = 'Power User' WHERE id = 2;
UPDATE user_classes SET name = 'Elite User' WHERE id = 3;
UPDATE user_classes SET name = 'Crazy User' WHERE id = 4;
UPDATE user_classes SET name = 'Insane User' WHERE id = 5;
UPDATE user_classes SET name = 'Veteran User' WHERE id = 6;
UPDATE user_classes SET name = 'Extreme User' WHERE id = 7;
UPDATE user_classes SET name = 'Ultimate User' WHERE id = 8;
UPDATE user_classes SET name = 'Nexus Master' WHERE id = 9;
-- 10-12 植物系余位（硕果/森林/生态）：NP 十级之外本站留作站长自定义扩展位，
-- 门槛拉满 1PB 级防误晋升（晋升仍可手動任命，不属成长线）
UPDATE user_classes SET name = 'Legend' WHERE id = 10;
UPDATE user_classes SET name = 'Overseer' WHERE id = 11;
UPDATE user_classes SET name = 'Immortal' WHERE id = 12;

-- 2) 门槛四维（NP 梯度：50G/4w → 3T/100w；seed_hours 温和比例自定）
DELETE FROM class_rules WHERE class_id BETWEEN 0 AND 12;
INSERT INTO class_rules
  (class_id, name, min_uploaded, min_download_count, min_seed_hours,
   min_account_age_days, demotable)
VALUES
  -- class 0 Peasant：降级落点，无门槛
  (0,  'Peasant',      0,               0,    0,    0,    false),
  (1,  'User',         0,               0,    0,    0,    false),
  -- NP: 4 周 / 50GB —— seed_hours 换算约 240h（4 周里日均 1h 做种）
  (2,  'Power User',   53687091200,     0,    240,  28,   true),
  -- NP: 8 周 / 120GB
  (3,  'Elite User',   128849018880,    0,    560,  56,   true),
  -- NP: 15 周 / 300GB
  (4,  'Crazy User',   322122547200,    0,    1120, 105,  true),
  -- NP: 25 周 / 500GB
  (5,  'Insane User',  536870912000,    0,    2000, 175,  true),
  -- NP: 40 周 / 750GB
  (6,  'Veteran User', 805306368000,    0,    3360, 280,  true),
  -- NP: 60 周 / 1TB
  (7,  'Extreme User', 1099511627776,   0,    5280, 420,  true),
  -- NP: 80 周 / 1.5TB
  (8,  'Ultimate User',1649267441664,   0,    7680, 560,  true),
  -- NP: 100 周 / 3TB
  (9,  'Nexus Master', 3298534883328,   0,    10000, 700, true),
  -- 扩展位（非 NP 官方线）：6TB / 12TB / 20TB，账龄 2/3/5 年
  (10, 'Legend',       6597069766656,   0,    14000, 730,  true),
  (11, 'Overseer',     13194139533312,  0,    18000, 1095, true),
  (12, 'Immortal',     21990232555520,  0,    22000, 1825, true);

COMMIT;
