-- 0412：把口粮券达标小时 `games_coupon_seed_hours` 登记进面板（P-469）
--
-- 现象：`apps/worker/src/jobs/game_coupons.rs` 读这把键（读不到时按 6 走），
-- 《PT游戏-口粮券与渔汛门槛-开发文档》§1 明写着「阈值走设置键
-- games_coupon_seed_hours（缺省 6），运维可热调，无需改代码重发」。
-- 但现网 `site_settings` 里**没有这一行**、`settings_meta` 里也没登记，
-- 于是面板上根本没有这一格：「热调」的唯一办法是直连数据库改 site_settings，
-- 而不知道这一点的运维会以为改代码才能调——文档承诺与可达能力分叉。
--
-- 补登记形状照同卡片的既有成员（`pet_feed_cost` 33 / `pet_digest_per_hour` 34
-- / `games_max_bet` 等都在 grp=module_fun 卡片里）：新行 card_order 取空位 35。
-- 区间取 0-24：语义是「一天里做种满几小时」，超过 24 无意义（结算侧 SQL 是
-- `HAVING count(DISTINCT 小时段) >= 阈值`，阈值调大=更严格）。
-- 注意：写 0 **不是关闭发放**——当日只要有一条 seeding_reward 流水即达标。
-- 结算侧没有「0=关」这条分支，标签里就不写这个承诺（代码做不到的事不写进面板）。
--
-- 顺序：`settings_meta.name` 有外键指向 `site_settings.name` ⇒ 先补值行再补元数据。
-- 幂等：两侧都是 ON CONFLICT DO NOTHING（站方若已自行加过，不覆盖其值与标签）。

INSERT INTO site_settings (name, value, descr, grp)
VALUES ('games_coupon_seed_hours', '6',
        '每日做种满几小时送 1 张口粮券（调大=更严格）', 'module_fun')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, min, max,
   group_key, card_order, visible, secret)
VALUES
  ('games_coupon_seed_hours', 'number', '口粮券达标做种小时',
   'Ration coupon seeding hours',
   '当日 seeding_reward 流水覆盖到的小时数达到此值即送 1 张口粮券；'
   '调大更难达标。0 不等于关闭：当天只要有一条做种奖励流水即达标。'
   '结算侧读同一把键，缺行时按 6。',
   '小时', 0, 24, 'module_fun', 35, true, false)
ON CONFLICT (name) DO NOTHING;
