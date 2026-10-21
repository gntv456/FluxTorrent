-- 0413：再补两把「代码在读、库里无行、面板无登记」的玩法参数（P-470）
--
-- 由 0412 同一把尺扫出来的（口粮券键修完后，把「帮助函数式读法」
-- `eco_i64(state, "key", 缺省)` 也纳入检测才看见它们——只认 SQL 里
-- `name = '字面量'` 的写法会漏掉整类调用）：
--
--   farm_fertilize_spark            apps/api/src/games_http/farm_actions.rs:240
--       施肥一次扣几火花，代码缺省 5。同族的 farm_water_spark（浇水）早就在
--       面板里（module_fun/18，[0,100] 魔力），只漏了这一把 ⇒ 一家注册了一半，
--       按本仓自己的口径就是漏登记。
--   games_fishing_event_seed_hours  apps/api/src/games_http/fishing_extra.rs:57
--       渔汛（周末活动塘）要求「本周做种满几小时」，代码缺省 20。
--       《口粮券与渔汛门槛》开发文档把它写成可热的门槛（周界用
--       `date_trunc('week', now() AT TIME ZONE 'Asia/Shanghai') AT TIME ZONE
--       'Asia/Shanghai'`，形状正确），但站长今天改不动它——只能改代码。
--
-- 两把都是「现网 site_settings 里连行都没有」⇒ 面板摸不到、生效值就是代码缺省。
-- 补法与 0412 相同：先插值行（值 = 代码缺省，grp=module_fun 与同家成员同卡片），
-- 再登记 settings_meta。card_order 取空位：施肥 19（紧跟浇水 18）、
-- 渔汛门槛 36（紧跟口粮券 35）。
-- 区间理由：施肥照浇水同档 [0,100] 魔力（同一动作的两兄弟，界不同只会让人困惑）；
-- 渔汛门槛 [0,168] 小时（一周的总小时数，填更大等于永久关塘，
-- 真要关请用 fishing_event_enabled 那把开关）。
--
-- 第三把 `fishing_event_enabled`（渔汛开关）现网已有行（值 '1'），只是没登记；
-- 上面那条提示要指它，就得让它同样可达，否则提示指向一个改不动的控件。
-- 类型必须是 number[0,1] 而不是 yesno：读侧是 `eco_i64(...) == 1`，
-- 走 `value::bigint` 转换——登记成 yesno 会让面板写进 'yes'，
-- 那一读就把开关读成「关」（或整条查询报错回落缺省），是个陷阱。
--
-- 幂等：两侧都是 ON CONFLICT DO NOTHING（站方若已自行加过，不覆盖其值与标签）。

INSERT INTO site_settings (name, value, descr, grp)
VALUES
  ('farm_fertilize_spark', '5',
   '农场施肥一次扣多少火花（0 = 免费）', 'module_fun'),
  ('games_fishing_event_seed_hours', '20',
   '渔汛周末活动塘要求本周做种满几小时', 'module_fun')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, min, max,
   group_key, card_order, visible, secret)
VALUES
  ('farm_fertilize_spark', 'number', '农场施肥扣火花',
   'Fertilize spark cost',
   '给一茬作物施肥收取的火花；与浇水同卡片、同区间口径。0 = 免费。',
   '魔力', 0, 100, 'module_fun', 19, true, false),
  ('games_fishing_event_seed_hours', 'number', '渔汛达标做种小时',
   'Fishing event seeding hours',
   '周末渔汛窗口内，本周做种小时数达到此值才能进活动塘；'
   '拒绝时会把「还差几小时」写给用户。关塘请改用「渔汛开关」那把（本迁移一并登记）。',
   '小时', 0, 168, 'module_fun', 36, true, false),
  ('fishing_event_enabled', 'number', '渔汛开关',
   'Fishing event enabled',
   '1 = 周末开活动塘（仍要达标），0 = 关。读侧是整数比较，'
   '只能填 0/1，不能用是/否型。',
   NULL, 0, 1, 'module_fun', 37, true, false)
ON CONFLICT (name) DO NOTHING;
